use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::sql::Thing;

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
pub struct IntervUserPrev {
    pub id: Thing,
    pub pass: String,
    pub role: String,
    pub state: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IntervUser {
    pub id: Thing,
    pub pass: String,
    pub role: String,
    pub state: UserState,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum UserState {
    Active,
    Exited,
    Standby,
    Completed,
}

impl From<IntervUserPrev> for IntervUser {
    fn from(user: IntervUserPrev) -> IntervUser {
        IntervUser {
            id: user.id,
            pass: user.pass,
            role: user.role,
            state: user.state.into(),
        }
    }
}

impl From<UserState> for String {
    fn from(state: UserState) -> String {
        match state {
            UserState::Active => "active".to_string(),
            UserState::Exited => "exited".to_string(),
            UserState::Standby => "standby".to_string(),
            UserState::Completed => "completed".to_string(),
        }
    }
}

impl From<String> for UserState {
    fn from(state: String) -> UserState {
        match state.as_ref() {
            "active" => UserState::Active,
            "exited" => UserState::Exited,
            "standby" => UserState::Standby,
            "completed" => UserState::Completed,
            _ => UserState::Standby,
        }
    }
}
