use futures::stream::StreamExt;
use serde_json::Value;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::{Notification, Surreal};

use crate::models::join::Join;
use crate::models::user::{IntervUser, IntervUserPrev};
use crate::modules::projects::manager::Credentials;

pub struct JoinManager {
    db: Surreal<Any>,
    cred: Credentials,
}

impl JoinManager {
    pub async fn new(url: impl Into<String>, cred: Credentials) -> Self {
        let url: String = url.into();

        let db = any::connect(format!("ws://{}", url))
            .await
            .expect("Failed to connect to database");

        let foo = Root {
            username: cred.user.as_str(),
            password: cred.pass.as_str(),
        };

        db.signin(foo)
        .await
        .expect("Failed to signin");

        db.use_ns("global")
            .use_db("main")
            .await
            .expect("Failed to use ns and db");

        Self { db, cred }
    }

    pub async fn start(&self) -> Result<(), &str> {
        // self.init_existing()
        //     .await
        //     .map_err(|_| "Failed to init existing projects")?;

        let mut stream = self
            .db
            .select("join")
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

    // async fn init_existing(&self) -> Result<(), &str> {
    //     // not sure if should be implemented
    //     Ok(())
    // }

    async fn handle_actions(&self, notification: Notification<Join>) -> Result<(), &str> {
        let join = notification.data;

        match notification.action {
            surrealdb::Action::Create => {
                // println!("Join created: {}", join.id);

                let mut res = self
                    .db
                    .query(
                        r#"
                        RETURN rand::ulid();
                        SELECT VALUE role FROM ONLY $b_user_id;
                        SELECT name, center.name as center FROM ONLY $b_project_id;"#,
                    )
                    .bind(("b_project_id", &join.project))
                    .bind(("b_user_id", &join.user))
                    .await
                    .unwrap();

                let center_project: (String, String) = res
                    .take(res.num_statements() - 1)
                    .map(|row: Option<Value>| {
                        let row = row.unwrap();

                        let name = row["name"].to_string().trim_matches('"').to_string();
                        let center = row["center"].to_string().trim_matches('"').to_string();

                        (center, name)
                    })
                    .unwrap();

                let role: Option<String> = res.take(res.num_statements() - 1).unwrap();
                let pass: Option<String> = res.take(res.num_statements() - 1).unwrap();

                if role.is_none() || pass.is_none() {
                    eprintln!("Failed to get role or pass");
                    return Ok(());
                }

                let role = role.unwrap();
                let pass = pass.unwrap();

                let sql = format!(
                    r#"
                    USE NS {} DB {};
                    CREATE $b_user_id SET role = $b_role, pass = $b_pass;"#,
                    center_project.0, center_project.1
                );

                let query = self
                    .db
                    .query(sql)
                    .bind(("b_user_id", &join.user))
                    .bind(("b_role", &role))
                    .bind(("b_pass", &pass));

                match query.await {
                    Ok(mut res) => {
                        let inter_user: IntervUser = res
                            .take(res.num_statements() - 1)
                            .map(|row: Option<IntervUserPrev>| {
                                let row = row.unwrap();

                                IntervUser {
                                    id: row.id,
                                    pass: row.pass,
                                    role: row.role,
                                    state: row.state.into(),
                                }
                            })
                            .map_err(|e| {
                                eprintln!("Failed to get interv_user: {}", e);
                            }).unwrap();

                        let state: String = inter_user.state.into();

                        self.db
                            .query(
                                r#"
                                UPDATE $b_join_id SET state = $b_state;
                                "#,
                            )
                            .bind(("b_join_id", &join.id))
                            .bind(("b_state", &state))
                            .await
                            .unwrap();

                        println!(
                            "User {} join project {} with pass:\n-> {}",
                            join.user,
                            join.project,
                            pass
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to create interv_user: {}", e);
                    }
                }
            }
            surrealdb::Action::Update => {
                // println!("Join updated: {}", join.id);

                if let Some(state) = join.state {
                    if state == "completed" {
                        self.db
                            .query(r#"UPDATE $b_user_id SET project = NONE;"#)
                            .bind(("b_user_id", &join.user))
                            .await
                            .unwrap();
                    }
                };
            }
            surrealdb::Action::Delete => { /* println!("Join deleted: {}", join.id) */ }
            _ => println!("Action not supported"),
        }

        Ok(())
    }
}
