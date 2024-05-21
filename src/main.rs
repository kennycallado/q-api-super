mod models;
mod modules;

use modules::projects::manager::Credentials;
use surrealdb::opt::auth::Root;

use crate::modules::join::manager::JoinManager;
use crate::modules::projects::manager::ProjectsManager;
// use crate::modules::users::manager::UserManager;

#[tokio::main]
async fn main() {
    let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost:8000".to_string());
    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "".to_string());

    let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "root".to_string());
    let db_pass = std::env::var("DB_PASS").unwrap_or_else(|_| "root".to_string());

    let db_url = if db_port.is_empty() {
        db_host
    } else {
        format!("{}:{}", db_host, db_port)
    };

    let cred = Credentials {
        user: db_user,
        pass: db_pass,
    };

    // let u_manager = UserManager::new(&db_url).await;
    let j_manager = JoinManager::new(&db_url, cred.clone()).await;
    let mut p_manager = ProjectsManager::new(&db_url, cred).await;

    println!("Listening for changes...");
    println!("Press Ctrl+C to stop.");

    match tokio::join!(j_manager.start(), p_manager.start()) {
        (Ok(_), Ok(_)) => {}
        (Err(e), _) => {
            eprintln!("Error in JoinManager: {}", e);
        }
        (_, Err(e)) => {
            eprintln!("Error in ProjectsManager: {}", e);
        }
    }
}
