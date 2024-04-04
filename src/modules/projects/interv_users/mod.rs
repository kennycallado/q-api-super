use futures::stream::StreamExt;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::{Notification, Surreal};

use crate::models::user::IntervUser;
use crate::modules::projects::manager::ProjectsManagerTrait;

#[derive(Clone)]
pub struct IntervUsersManager {
    db: Surreal<Any>,
}

impl IntervUsersManager {
    pub async fn new(url: impl Into<String>) -> Self {
        let db = any::connect(format!("ws://{}", url.into()))
            .await
            .expect("Failed to connect to database");

        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await
        .expect("Failed to signin");

        Self { db }
    }

    fn spawn_stream(&self, center: impl Into<String>, project: impl Into<String>) {
        let manager = self.clone();
        let project = project.into();
        let center = center.into();

        tokio::spawn(async move {
            let manager = manager.clone();

            // WARNING: not sure if it's ok
            manager.db.use_ns(&center).use_db(&project).await.unwrap();

            let mut users_stream = manager.db.select("users").live().await.unwrap();
            while let Some(result) = users_stream.next().await {
                match result {
                    Ok(notification) => {
                        manager
                            .handle_actions(center.as_str(), project.as_str(), notification)
                            .await;
                    }
                    Err(error) => eprintln!("{error}"),
                }
            }
        });
    }

    async fn handle_actions(
        &self,
        _center: &str,
        _project: &str,
        notification: Notification<IntervUser>,
    ) {
        let user = notification.data;

        match notification.action {
            surrealdb::Action::Create => { /* println!("User created: {}", user.id) */ }
            surrealdb::Action::Update => {
                println!("User updated: {}", user.id);

                if user.completed {
                    self.db
                        .query(r#"
                            LET $q_score = SELECT VALUE score FROM ONLY (
                                SELECT created, score FROM ONLY scores WHERE user IS $b_id ORDER BY created DESC LIMIT 1
                            ) LIMIT 1;

                            USE NS global DB main;

                            BEGIN TRANSACTION;
                                UPDATE join SET completed = true, updated = time::now(), score = $q_score WHERE in IS $b_id;
                                UPDATE $b_id SET project = NONE; -- should be done by join events
                            COMMIT TRANSACTION;
                        "#)
                        .bind(("b_id", user.id))
                        .await
                        .unwrap();
                }
            }
            surrealdb::Action::Delete => { /* println!("User deleted: {}", user.id) */ }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl ProjectsManagerTrait for IntervUsersManager {
    async fn on_init(&self, project: &str, center: &str) {
        self.spawn_stream(center, project);
    }

    async fn on_project_create(&self, project: &str, center: &str) {
        self.spawn_stream(center, project);
    }

    async fn on_project_update(&self, _project: &str) {}
    async fn on_project_delete(&self, _project: &str) {}
}
