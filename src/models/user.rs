use serde::Deserialize;
use serde_json::Value;
use surrealdb::sql::Thing;

// #[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(Debug, Deserialize)]
pub struct User {
    pub id: Thing,
    pub role: String,
    pub username: String,
    pub password: String,
    pub project: Option<Thing>,
    pub web_token: Value,
}

#[derive(Debug, Deserialize)]
pub struct IntervUser {
    pub id: Thing,
    pub pass: String,
    pub role: String,
    pub active: bool,
    pub completed: bool,
}
