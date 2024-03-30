use std::sync::Arc;

use futures::stream::StreamExt;
use surrealdb::engine::any::{self, Any};
use surrealdb::engine::remote::http::Http;
use surrealdb::opt::auth::Root;
use surrealdb::{Notification, Surreal};

use crate::models::center::Center;
use crate::models::project::Project;

struct Listener(pub Arc<dyn ProjectsManagerTrait>);

pub struct ProjectsManager {
    db: Surreal<Any>,
    db_url: String,
    listeners: Vec<Listener>,
}

impl ProjectsManager {
    pub async fn new(url: String) -> Self {
        let db = any::connect(format!("ws://{}", url))
            .await
            .expect("Failed to connect to database");

        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await
        .expect("Failed to signin");

        db.use_ns("global")
            .use_db("main")
            .await
            .expect("Failed to use ns and db");

        Self {
            db,
            db_url: url,
            listeners: vec![],
        }
    }

    pub fn add_listener(&mut self, f: impl ProjectsManagerTrait + 'static) {
        self.listeners.push(Listener(Arc::new(f)));
    }

    pub async fn start(&mut self) -> Result<(), &str> {
        self.init_existing()
            .await
            .map_err(|_| "Failed to init existing projects")?;

        let mut stream = self
            .db
            .select("projects")
            .live()
            .await
            .map_err(|_| "Failed to get projects")?;

        while let Some(result) = stream.next().await {
            match result {
                Ok(notification) => {
                    if let Err(error) = self.handle_actions(notification).await {
                        eprintln!("{error}");
                    }
                }

                Err(error) => eprintln!("{error}"),
            }
        }

        Ok(())
    }

    async fn init_existing(&self) -> Result<(), &str> {
        let projects: Vec<Project> = self
            .db
            .select("projects")
            .await
            .map_err(|_| "Failed to get projects")?;

        for project in projects {
            let center: Option<Center> = self
                .db
                .select(&project.center)
                .await
                .map_err(|_| "Failed to get center")?;

            let center = center.unwrap();

            for handler in &self.listeners {
                handler.0.on_init(&project.name, &center.name).await;
            }
        }

        Ok(())
    }

    async fn handle_actions(&self, notification: Notification<Project>) -> Result<(), &str> {
        let project = notification.data;

        match notification.action {
            surrealdb::Action::Create => {
                let center: Option<Center> = self
                    .db
                    .select(&project.center)
                    .await
                    .map_err(|_| "Failed to get center")?;
                let center = center.unwrap();

                self.execute_migrations(&center.name, &project.name);

                // {{{ info for sc user
                let sql = format!("
                    USE NS {} DB {};
                    DEFINE TOKEN user_scope ON SCOPE user TYPE HS256 VALUE '{}';
                    ", &center.name, &project.name, &project.token);

                self.db.query(sql).await.unwrap();
                // }}}

                for handler in &self.listeners {
                    handler.0.on_project_create(&project.name, &center.name).await;
                }
            }
            surrealdb::Action::Update => {
                for handler in &self.listeners {
                    handler.0.on_project_update(&project.name).await;
                }
            }
            surrealdb::Action::Delete => {
                // TODO: backup project data

                for handler in &self.listeners {
                    handler.0.on_project_delete(&project.name).await;
                }
            }
            _ => println!("Action not supported"),
        }

        Ok(())
    }

    fn execute_migrations(&self, center_name: impl Into<String>, project_name: impl Into<String>) {
        // TODO:
        // - url
        // - credentials

        let center_name = center_name.into();
        let project_name = project_name.into();
        let p_db = self.db.clone();
        let db_url = self.db_url.clone();

        tokio::spawn(async move {
            let db = Surreal::new::<Http>(db_url).await.unwrap();
            db.signin(Root {
                username: "root",
                password: "root",
            })
            .await
            .unwrap();

            db.use_ns("global").use_db("interventions").await.unwrap();
            let mut backup = db.export(()).await.unwrap();

            let mut buffer = Vec::new();
            while let Some(result) = backup.next().await {
                if let Ok(data) = result {
                    buffer.extend_from_slice(&data);
                }
            }

            let sql = format!(
                "USE NS {} DB {}; {};",
                center_name,
                project_name,
                String::from_utf8(buffer).unwrap()
            );
            p_db.query(sql).await.unwrap();
        });
    }
}

#[async_trait::async_trait]
pub trait ProjectsManagerTrait: Send + Sync + 'static {
    async fn on_init(&self,           project: &str, center: &str);
    async fn on_project_create(&self, project: &str, center: &str);
    async fn on_project_update(&self, project: &str);
    async fn on_project_delete(&self, project: &str);
}
