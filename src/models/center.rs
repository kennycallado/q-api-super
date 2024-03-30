use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Center {
    pub id: Option<Thing>,
    pub name: String,
}
