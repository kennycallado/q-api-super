use futures::stream::StreamExt;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::{Notification, Surreal};

use crate::models::user::User;

pub struct UserManager {
    db: Surreal<Any>,
}

impl UserManager {
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

        db.use_ns("global")
            .use_db("main")
            .await
            .expect("Failed to use ns and db");

        Self { db }
    }

    pub async fn start(&self) -> Result<(), &str> {
        let mut stream = self
            .db
            .select("users")
            .live()
            .await
            .map_err(|_| "Failed to get users")?;

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

    async fn handle_actions(&self, notification: Notification<User>) -> Result<(), &str> {
        // let user = notification.data;

        match notification.action {
            surrealdb::Action::Create => { /* println!("User created: {}", user.username) */ }
            surrealdb::Action::Update => { /* println!("User updated: {}", user.username) */ }
            surrealdb::Action::Delete => { /* println!("User deleted: {}", user.username) */ }
            _ => println!("Action not supported"),
        }

        Ok(())
    }
}
