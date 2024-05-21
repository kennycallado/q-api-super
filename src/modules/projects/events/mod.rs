use std::sync::Arc;

use futures::stream::StreamExt;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use surrealdb::sql::Thing;
use surrealdb::{Notification, Surreal};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

use crate::modules::projects::manager::ProjectsManagerTrait;

use crate::models::event::Event;

use super::manager::Credentials;

#[derive(Clone)]
pub struct EventsManager {
    db: Surreal<Any>,
    cred: Credentials,
    sched: Arc<Mutex<JobScheduler>>,
}

impl EventsManager {
    pub async fn new(url: impl Into<String>, cred: Credentials) -> Self {
        let sched = JobScheduler::new().await.unwrap();
        sched.start().await.unwrap();

        let db = any::connect(format!("ws://{}", url.into()))
            .await
            .expect("Failed to connect to database");

        let foo = Root {
            username: cred.user.as_str(),
            password: cred.pass.as_str(),
        };

        db.signin(foo).await.expect("Failed to signin");

        Self {
            db,
            cred,
            sched: Arc::new(Mutex::new(sched)),
        }
    }

    fn spawn_stream(&self, center: impl Into<String>, project: impl Into<String>) {
        let manager = self.clone();
        let project = project.into();
        let center = center.into();

        tokio::spawn(async move {
            let manager = manager.clone();

            // WARNING: not sure if it's ok
            manager.db.use_ns(&center).use_db(&project).await.unwrap();

            let mut events_stream = manager.db.select("events").live().await.unwrap();
            while let Some(result) = events_stream.next().await {
                match result {
                    Ok(notification) => {
                        manager
                            .handle_actions(center.as_str(), project.as_str(), notification)
                            .await;
                    }
                    Err(error) => eprintln!("{error}"),
                }
            }
        });
    }

    async fn select_events(&self, center: &str, project: &str) -> Vec<Event> {
        let sql = format!("USE NS {} DB {}; SELECT * FROM events;", center, project);
        let mut res = self.db.query(sql).await.unwrap();

        res.take(res.num_statements() - 1).unwrap()
    }

    async fn select_event(&self, center: &str, project: &str, id: &Thing) -> Option<Event> {
        let sql = format!("USE NS {} DB {}; SELECT * FROM $b_id;", center, project);
        let mut res = self.db.query(sql).bind(("b_id", id)).await.unwrap();

        res.take(res.num_statements() - 1).unwrap()
    }

    async fn update_event(&self, center: &str, project: &str, event: Event) {
        let id = event.id.as_ref().unwrap().clone();

        let sql = format!(
            "USE NS {} DB {}; UPDATE $b_id CONTENT $b_content;",
            center, project
        );

        self.db
            .query(sql)
            .bind(("b_id", id))
            .bind(("b_content", event))
            .await
            .unwrap();
    }

    async fn handle_actions(&self, center: &str, project: &str, notification: Notification<Event>) {
        let mut event = notification.data;

        match notification.action {
            surrealdb::Action::Create => {
                self.create_job(&mut event, center, project).await.unwrap();
                self.update_event(center, project, event).await;
            }
            surrealdb::Action::Update => {
                if event.job_id.is_none() && event.active {
                    self.create_job(&mut event, center, project).await.unwrap();
                    self.update_event(center, project, event).await;
                } else if event.job_id.is_some() && !event.active && event.status.is_none() {
                    event.status = Some("scheduled".to_string());
                    self.update_event(center, project, event).await;
                }
            }
            surrealdb::Action::Delete => {
                // println!("In center: {} and {}", center, project);
                // println!("Event deleted: {:?}", event);
            }
            _ => {}
        }
    }

    pub async fn create_job(
        &self,
        event: &mut Event,
        center: &str,
        project: &str,
    ) -> Result<(), JobSchedulerError> {
        let event_shedule = event.schedule.clone();
        let center = center.to_string();
        let manager = self.clone();
        let project = project.to_string();
        let id = event.id.as_ref().unwrap().clone();

        let job = Job::new_async(event_shedule.as_str(), move |uuid, lock| {
            let id = id.clone();
            let manager = manager.clone();
            let center = center.to_string();
            let project = project.to_string();

            Box::pin(async move {
                manager
                    .handle_status(id, uuid, lock, center.as_str(), project.as_str())
                    .await
            })
        })?;

        let scheduler = self.sched.lock().await;
        let res = scheduler.add(job).await?;
        drop(scheduler); // alternative to scope

        event.job_id = Some(res.into());
        event.status = Some("scheduled".to_string());

        Ok(())
    }

    pub async fn event_execute(&self, center: &str, project: &str, script: &str) -> Result<(), ()> {
        let sql = format!(
            "USE NS {} DB {}; fn::on_cron('{}');",
            center, project, script
        );
        let res = self.db.query(sql).await;

        match res {
            Ok(mut r) => {
                let _: Option<String> = r.take(r.num_statements() - 1).map_err(|e| {
                    eprintln!(
                        "Error: {:?};\ncenter = {center} -> project = {project}: script = {script}",
                        e
                    )
                })?;

                Ok(())
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);

                Err(())
            }
        }
    }

    pub async fn handle_status(
        &self,
        id: Thing,
        uuid: uuid::Uuid,
        mut lock: JobScheduler,
        center: &str,
        project: &str,
    ) {
        if let Some(mut event) = self.select_event(center, project, &id).await {
            if !event.active {
                event.status = Some("scheduled".to_string());
                self.update_event(center, project, event).await;

                return;
            }

            self.event_check(&mut event, &mut lock, uuid).await;

            let pre = self
                .event_execute(center, project, event.script.as_str())
                .await;

            match pre {
                Ok(_) => {}
                Err(_) => {
                    event.status = Some("failed".to_string());
                    event.active = false;

                    match self.sched.lock().await.remove(&uuid).await {
                        Ok(_) => event.job_id = None,
                        Err(e) => eprintln!("Error: {e:?}\n;"),
                    }
                }
            }

            self.update_event(center, project, event).await;
        }
    }

    pub async fn event_check(&self, event: &mut Event, lock: &mut JobScheduler, uuid: uuid::Uuid) {
        let next_tick = match lock.next_tick_for_job(uuid).await {
            Ok(Some(ts)) => Some(ts),
            Ok(None) => {
                println!("Job Done!");

                event.status = Some("done".to_string());
                event.active = false;

                None
            }
            _ => {
                // unreachable
                eprintln!("Could not get next tick from job");

                return;
            }
        };

        if let Some(ts) = next_tick {
            if let Some(since) = event.since.clone() {
                if *since > ts {
                    return;
                }
            }

            if let Some(until) = event.until.clone() {
                if *until < ts {
                    println!("Job Done!");

                    event.status = Some("done".to_string());
                    event.active = false;
                }
            }
        }

        if let Some(status) = event.status.clone() {
            if status == "scheduled" {
                event.status = Some("running".to_string());
            }
            // if status == "running" {}
            // if status == "failed" {}
            // if status == "done" {}
        };
    }
}

#[async_trait::async_trait]
impl ProjectsManagerTrait for EventsManager {
    async fn on_init(&self, project: &str, center: &str) {
        let events = self.select_events(center, project).await;
        for mut event in events {
            event.job_id = None;

            if let Some(ref status) = event.status {
                if status != "done" && status != "failed" {
                    self.create_job(&mut event, center, project).await.unwrap();
                }
            }

            self.update_event(center, project, event).await;
        }

        self.spawn_stream(center, project);
    }

    async fn on_project_create(&self, project: &str, center: &str) {
        self.spawn_stream(center, project);
    }

    async fn on_project_update(&self, _project: &str) {}
    async fn on_project_delete(&self, _project: &str) {}
}
