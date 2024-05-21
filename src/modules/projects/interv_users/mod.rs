use futures::stream::StreamExt;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::{Notification, Surreal};

use crate::models::user::{IntervUser, IntervUserPrev, UserState};
use crate::modules::projects::manager::ProjectsManagerTrait;

use super::manager::Credentials;

#[derive(Clone)]
pub struct IntervUsersManager {
    db: Surreal<Any>,
    cred: Credentials
}

impl IntervUsersManager {
    pub async fn new(url: impl Into<String>, cred: Credentials) -> Self {
        let db = any::connect(format!("ws://{}", url.into()))
            .await
            .expect("Failed to connect to database");

        let foo = Root {
            username: cred.user.as_str(),
            password: cred.pass.as_str(),
        };

        db.signin(foo)
        .await
        .expect("Failed to signin");

        Self { db, cred }
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
        center: &str,
        project: &str,
        notification: Notification<IntervUserPrev>,
    ) {
        let user: IntervUser = notification.data.into();

        match notification.action {
            surrealdb::Action::Update => {
                // println!("User updated: {}", user.id);

                match user.state {
                    UserState::Completed | UserState::Exited => {
                        let state: String = user.state.into();

                        self.db.use_ns(center).use_db(project).await.unwrap();
                        self.db
                            .query(r#"
                                LET $q_score = SELECT VALUE score FROM ONLY (
                                    SELECT created, score FROM ONLY scores WHERE user IS $b_id ORDER BY created DESC LIMIT 1
                                ) LIMIT 1;

                                USE NS global DB main;

                                BEGIN TRANSACTION;
                                    UPDATE join SET state = $b_state, score = $q_score, updated = time::now() WHERE in IS $b_id;
                                    UPDATE $b_id SET project = NONE; -- should be done by join events
                                COMMIT TRANSACTION;
                            "#)
                            .bind(("b_id", user.id))
                            .bind(("b_state", state))
                            .await
                            .unwrap();
                    },
                    UserState::Active | UserState::Standby => {
                        let state: String = user.state.into();

                        self.db.use_ns("global").use_db("main").await.unwrap();
                        self.db
                            .query("UPDATE join SET state = $b_state, updated = time::now() WHERE in IS $b_id;")
                            .bind(("b_id", user.id))
                            .bind(("b_state", state))
                            .await
                            .unwrap();
                    },
                    _ => { }
                }
            }
            surrealdb::Action::Create => { /* println!("User created: {}", user.id) */ }
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
