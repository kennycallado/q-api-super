use tokio_cron_scheduler::{JobScheduler, JobSchedulerError};

pub async fn init_scheduler() -> Result<JobScheduler, JobSchedulerError> {
    let sched = JobScheduler::new().await?;
    sched.start().await?;

    Ok(sched)
}

async fn project_init_existing(sched: Arc<Mutex<JobScheduler>>) -> surrealdb::Result<()> {
    let projects: Vec<Project> = G_DB.select("projects").await?;
    for project in projects {
        let events = events_select(&project.name).await?;
        for mut event in events {
            event.job_id = None;

            // if the event was active and the status is not done or failed, then spawn
            if event.status.is_some() {
                match event.status {
                    Some(ref status) => {
                        if status != "done" && status != "failed" {
                            jobs_create(sched.clone(), &mut event, &project.name)
                                .await
                                .expect("Failed to create job");
                        }

                        // if event.active && status != "done" && status != "failed" {
                        //     create_job(sched.clone(), &mut event, &project.name).await.expect("Failed to create job");
                        // }

                        // create_job(sched.clone(), &mut event, &project.name).await.expect("Failed to create job");
                    }
                    None => {}
                }
            }

            // update the event
            event_update(&project.name, event).await?;
        }

        // spawn
        events_spawn_stream(sched.clone(), project);
    }

    Ok(())
}

async fn projects_handle_actions(
    sched: Arc<Mutex<JobScheduler>>,
    notification: Notification<Project>,
) {
    let project = notification.data;

    match notification.action {
        surrealdb::Action::Create => {
            println!("Project updated: {}", project.name);

            events_spawn_stream(sched, project);
        }
        surrealdb::Action::Update => println!("Project updated: {}", project.name),
        surrealdb::Action::Delete => println!("Project deleted: {}", project.name), // ensure that the events are removed from the scheduler
        _ => println!("Unknown action: {:?}", notification.action),
    }
}

fn events_spawn_stream(sched: Arc<Mutex<JobScheduler>>, project: Project) {
    tokio::spawn(async move {
        // WARNING:
        // TODO: checks if it is ok
        let mut events_stream: Stream<'_, Client, Vec<Event>>;
        {
            I_DB.use_db(project.name.as_str()).await.unwrap();
            events_stream = I_DB.select("events").live().await.unwrap();
        }

        while let Some(result) = events_stream.next().await {
            match result {
                Ok(notification) => {
                    events_handle_action(sched.clone(), notification, project.name.clone()).await
                }
                Err(error) => eprintln!("{error}"),
            }
        }
    });
}

async fn events_handle_action(
    sched: Arc<Mutex<JobScheduler>>,
    notification: Notification<Event>,
    p_name: String,
) {
    let mut event = notification.data;

    match notification.action {
        surrealdb::Action::Create => {
            println!("Event created: {}", event.id.clone().unwrap());

            jobs_create(sched.clone(), &mut event, p_name.clone())
                .await
                .expect("Failed to create job");

            event_update(p_name, event)
                .await
                .expect("Failed to update event");
        }
        surrealdb::Action::Update => {
            println!("Event updated: {}", event.id.clone().unwrap());

            if event.job_id.is_none() && &event.active {
                jobs_create(sched.clone(), &mut event, p_name.clone())
                    .await
                    .expect("Failed to create job");
                event_update(p_name, event)
                    .await
                    .expect("Failed to update event");
            } else if event.job_id.is_some() && !event.active && event.status.is_none() {
                event.status = Some("scheduled".to_string());
                event_update(p_name, event)
                    .await
                    .expect("Failed to update event");
            }
        }
        surrealdb::Action::Delete => {
            println!("Event deleted: {}", event.id.unwrap());

            if event.job_id.is_some() {
                let scheduler = sched.lock().await;
                scheduler
                    .remove(&event.job_id.unwrap())
                    .await
                    .expect("Failed to remove job");
            }
        }
        _ => println!("Unknown action: {:?}", notification.action),
    }
}

async fn jobs_create(
    sched: Arc<Mutex<JobScheduler>>,
    event: &mut Event,
    p_name: impl Into<String>,
) -> Result<(), JobSchedulerError> {
    let event_shedule = event.schedule.clone();
    let event_id = event.id.clone().unwrap();
    let p_name = p_name.into();

    let job = Job::new_async(event_shedule.as_str(), move |uuid, lock| {
        let event_id = event_id.clone();
        let p_name = p_name.clone();

        Box::pin(async move { event_handle_status(uuid, lock, event_id, &p_name).await })
    })?;

    let scheduler = sched.lock().await;
    let res = scheduler.add(job).await?;
    drop(scheduler); // alternative to scope

    event.job_id = Some(res.into());
    event.status = Some("scheduled".to_string());

    Ok(())
}

async fn event_handle_status(
    uuid: uuid::Uuid,
    mut lock: JobScheduler,
    id: Thing,
    p_name: impl Into<String>,
) {
    let p_name = p_name.into();
    let mut event = event_select(p_name.clone(), id.clone())
        .await
        .expect("Failed to get event");

    if !event.active {
        event.status = Some("scheduled".to_string());
        event_update(p_name.clone(), event.clone())
            .await
            .expect("Failed to update event");

        return;
    }
    event_checks(&mut lock, uuid, &mut event).await;

    event_execute(&p_name, &mut event).await;
    event_update(p_name.clone(), event.clone())
        .await
        .expect("Failed to update event");
}

async fn event_checks(lock: &mut JobScheduler, uuid: uuid::Uuid, event: &mut Event) {
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
        if status == "running" {}
        if status == "failed" {}
        if status == "done" {}
    };
}

async fn event_execute(p_name: &str, event: &mut Event) {
    let sql = format!("USE db {}; {}", p_name, QUERY_EXECUTION); // change db and query at the same time

    let mut res = I_DB
        .query(sql)
        .bind(("b_script", &event.script))
        .await
        .expect("Failed to execute event");
    match res.take::<Option<String>>(res.num_statements() - 1) {
        Ok(_) => {}
        Err(e) => {
            event.status = Some("failed".to_string());
            event.active = false;

            eprintln!("{:?}", e);
        }
    }
}

async fn events_select(p_name: impl Into<String>) -> surrealdb::Result<Vec<Event>> {
    let events: Vec<Event>;
    let sql = format!("USE db {}; SELECT * FROM events;", p_name.into()); // change db and query at the same time
    let mut res = I_DB.query(sql).await?;
    events = res.take(res.num_statements() - 1)?;

    Ok(events)
}

async fn event_select(p_name: impl Into<String>, id: Thing) -> surrealdb::Result<Event> {
    let event: Event;
    let sql = format!("USE db {}; SELECT * FROM $b_id;", p_name.into()); // change db and query at the same time
    let mut res = I_DB.query(sql).bind(("b_id", &id)).await?;
    let p_event: Option<Event> = res.take(res.num_statements() - 1)?;
    event = p_event.expect("Failed to get event");

    Ok(event)
}

async fn event_update(p_name: impl Into<String>, event: Event) -> surrealdb::Result<Event> {
    let sql = format!("USE db {}; UPDATE $b_id CONTENT $b_content;", p_name.into()); // change db and query at the same time
    let mut res = I_DB
        .query(sql)
        .bind(("b_id", event.id.clone().unwrap()))
        .bind(("b_content", event))
        .await?;
    let event: Option<Event> = res.take(res.num_statements() - 1)?;
    let event = event.expect("Failed to update event");

    Ok(event)
}
