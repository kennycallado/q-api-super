mod models;
mod modules;

use crate::modules::join::manager::JoinManager;
use crate::modules::projects::manager::ProjectsManager;
// use crate::modules::users::manager::UserManager;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost:8000".to_string());

    // let u_manager = UserManager::new(&db_url).await;
    let j_manager = JoinManager::new(&db_url).await;
    let mut p_manager = ProjectsManager::new(&db_url).await;

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
