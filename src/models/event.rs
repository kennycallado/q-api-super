use serde::{Deserialize, Serialize};
use surrealdb::sql::{Datetime, Thing, Uuid};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Option<Thing>,
    pub active: bool,
    pub script: String,
    pub status: Option<String>,
    pub job_id: Option<Uuid>,
    pub schedule: String,
    pub since: Option<Datetime>,
    pub until: Option<Datetime>,
}
