use serde::Deserialize;
use surrealdb::sql::{Datetime, Thing};

// use super::user::UserState;

#[derive(Debug, Deserialize)]
pub struct Join {
    pub id: Thing,
    #[serde(rename = "in")]
    pub user: Thing,
    #[serde(rename = "out")]
    pub project: Thing,
    // pub state: Option<UserState>,
    pub state: Option<String>,
    pub created: Datetime,
    pub updated: Datetime,
}
