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

        db.signin(foo).await.expect("Failed to signin");

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

    async fn handle_actions(&self, notification: Notification<Join>) -> Result<(), &str> {
        let join = notification.data;

        match notification.action {
            surrealdb::Action::Create => {
                // println!("Join created: {}", join.id);

                let mut res = self
                    .db
                    .query(
                        r#"
                        SELECT
                            out.center.name as center,
                            out.name as name,
                            (<-users->roled[WHERE out IS $parent.out.center].role)[0] AS role
                            FROM ONLY $b_id;
                        "#,
                    )
                    .bind(("b_id", &join.id))
                    .await
                    .unwrap();

                let center_name_role: (String, String, String) = res
                    .take(res.num_statements() - 1)
                    .map(|row: Option<Value>| {
                        let row = row.unwrap();

                        let center = row["center"].to_string().trim_matches('"').to_string();
                        let name = row["name"].to_string().trim_matches('"').to_string();
                        let role = row["role"].to_string().trim_matches('"').to_string();

                        (center, name, role)
                    })
                    .map_err(|e| {
                        eprintln!("Failed to get interv_user: {}", e);
                        return "Error: interv_user";
                    })?;

                let center = center_name_role.0;
                let name = center_name_role.1;
                let role = center_name_role.2;

                match role.as_str() {
                    "parti" | "guest" => {}
                    _ => return Ok(()),
                }

                let sql = format!(
                    r#"
                    USE NS {} DB {};
                    CREATE $b_user_id SET role = $b_role;"#,
                    &center, &name
                );

                let query = self
                    .db
                    .query(sql)
                    .bind(("b_user_id", &join.user))
                    .bind(("b_role", &role));

                match query.await {
                    Ok(mut res) => {
                        let inter_user: Result<IntervUser, ()> = res
                            .take(res.num_statements() - 1)
                            .map(|row: Option<IntervUserPrev>| {
                                let row = row.unwrap();

                                IntervUser {
                                    id: row.id,
                                    role: row.role,
                                    state: row.state.into(),
                                }
                            })
                            .map_err(|e| {
                                eprintln!("Failed to get interv_user: {}", e);
                                return ();
                            });

                        if let Err(_) = inter_user {
                            eprintln!("Failed to get interv_user");
                            return Ok(());
                        }
                        let inter_user = inter_user.unwrap();

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

                        println!("User {} join project {} \n", join.user, join.project,);
                    }
                    Err(e) => {
                        eprintln!("Failed to create interv_user: {}", e);
                    }
                }
            }
            surrealdb::Action::Update => {
                // println!("Join updated: {}", join.id);

                // done by an event
                // if let Some(state) = join.state {
                //     if state == "completed" {
                //         self.db
                //             .query(r#"UPDATE $b_user_id SET project = NONE;"#)
                //             .bind(("b_user_id", &join.user))
                //             .await
                //             .unwrap();
                //     }
                // };
            }
            surrealdb::Action::Delete => { /* println!("Join deleted: {}", join.id) */ }
            _ => println!("Action not supported"),
        }

        Ok(())
    }
}
