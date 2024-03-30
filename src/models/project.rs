use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::center::Center;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: Option<Thing>,
    pub name: String,
    pub center: Thing,
    pub state: String,
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectWithCenter {
    pub id: Option<Thing>,
    pub name: String,
    pub center: Center,
    pub state: String,
    pub token: String,
}
