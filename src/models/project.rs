use serde::Deserialize;
use surrealdb::sql::Thing;

use super::center::Center;

#[derive(Debug, Deserialize)]
pub struct Project {
    pub id: Option<Thing>,
    pub name: String,
    pub center: Thing,
    pub state: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct ProjectWithCenter {
    pub id: Option<Thing>,
    pub name: String,
    pub center: Center,
    pub state: String,
    pub token: String,
}
