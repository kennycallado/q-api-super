use serde::{Deserialize};
use surrealdb::sql::{Datetime, Thing};

#[derive(Debug, Deserialize)]
pub struct Join {
    pub id: Thing,
    #[serde(rename = "in")]
    pub user: Thing,
    #[serde(rename = "out")]
    pub project: Thing,
    pub completed: bool,
    pub created: Datetime,
    pub updated: Datetime,
}
