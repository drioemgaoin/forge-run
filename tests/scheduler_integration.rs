use forge_run::application::context::AppContext;
use forge_run::application::usecases::scheduler::SchedulerUseCase;
use forge_run::config::Settings;
use forge_run::domain::entities::job::{Job, JobState};
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::domain::value_objects::ids::{ClientId, EventId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use std::sync::Arc;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn given_missed_schedule_when_run_once_should_queue_job() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let settings = Settings {
        server: forge_run::config::Server {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        db: forge_run::config::Db { url },
        redis: forge_run::config::Redis {
            url: "redis://127.0.0.1/".to_string(),
        },
        workers: forge_run::config::Workers {
            default_count: 1,
            max_count: 1,
            poll_interval_ms: 250,
            lease_timeout_seconds: 30,
            scale_interval_ms: 1000,
        },
        scheduler: forge_run::config::Scheduler {
            poll_interval_ms: 1000,
            max_batch: 100,
            skew_seconds: 1,
            tolerance_ms: 100,
        },
    };
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle), settings.clone());

    let execution_at =
        Timestamp::from(Timestamp::now_utc().as_inner() - time::Duration::seconds(5));
    let job = Job::new_deferred(
        JobId::new(),
        ClientId::new(),
        execution_at,
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    let stored = repos.job.insert(&job).await.unwrap();

    let count = SchedulerUseCase::run_once(&ctx, Timestamp::now_utc(), 10)
        .await
        .unwrap();

    assert_eq!(count, 1);
    let queued = repos.job.get(stored.id).await.unwrap().unwrap();
    assert_eq!(queued.state, JobState::Queued);

    let events = repos.event.list_by_job_id(stored.id).await.unwrap();
    for event in events {
        repos.event.delete(EventId(event.id)).await.unwrap();
    }
    repos.job.delete(stored.id).await.unwrap();
}
