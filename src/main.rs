mod models;
mod modules;
mod projects;

use crate::modules::events::manager::EventsManager;
use crate::projects::ProjectsManager;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost:8000".to_string());

    let mut p_manager = ProjectsManager::new(db_url.clone()).await;
    let e_manager = EventsManager::new(db_url).await;
    p_manager.add_listener(e_manager);

    println!("Listening for changes...");
    println!("Press Ctrl+C to stop.");

    tokio::join!(p_manager.start(), u_manager()).0.unwrap();
}

async fn u_manager() {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        println!("waiting for users implementation...");
    }
}
