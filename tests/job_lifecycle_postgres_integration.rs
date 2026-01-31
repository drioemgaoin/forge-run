use forge_run::domain::services::job_lifecycle::{JobLifecycle, JobLifecycleService};
use forge_run::domain::value_objects::ids::{ClientId, JobId};
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use std::sync::Arc;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn given_instant_job_when_created_should_persist_job_and_event() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());

    let (job, event) = lifecycle
        .create_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .await
        .unwrap();

    let stored_job = repos.job.get(job.id).await.unwrap();
    let stored_event = repos.event.get(event.id).await.unwrap();

    assert!(stored_job.is_some());
    assert!(stored_event.is_some());

    repos.event.delete(event.id).await.unwrap();
    repos.job.delete(job.id).await.unwrap();
}
