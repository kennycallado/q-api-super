use serde::Deserialize;
use surrealdb::sql::Thing;

#[derive(Debug, Deserialize)]
pub struct Center {
    pub id: Option<Thing>,
    pub name: String,
}
