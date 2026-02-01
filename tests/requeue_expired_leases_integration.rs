use forge_run::application::context::AppContext;
use forge_run::application::usecases::requeue_expired_leases::RequeueExpiredLeasesUseCase;
use forge_run::domain::entities::job::{Job, JobState};
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::domain::value_objects::ids::JobId;
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use std::sync::Arc;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn given_expired_lease_when_execute_should_requeue_job() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle));

    let mut job = Job::new_instant(
        JobId::new(),
        forge_run::domain::value_objects::ids::ClientId::new(),
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    job.state = JobState::Assigned;
    job.lease_owner = Some("worker-1".to_string());
    job.lease_expires_at = Some(Timestamp::from(
        Timestamp::now_utc().as_inner() - time::Duration::seconds(30),
    ));
    job.updated_at = Timestamp::now_utc();
    repos.job.insert(&job).await.unwrap();

    let result = RequeueExpiredLeasesUseCase::execute(&ctx, Timestamp::now_utc(), 10)
        .await
        .unwrap();

    assert_eq!(result.jobs.len(), 1);
    let requeued = &result.jobs[0];
    assert_eq!(requeued.id, job.id);
    assert_eq!(requeued.state, JobState::Queued);
    assert!(requeued.lease_owner.is_none());

    let events = repos.event.list_by_job_id(job.id).await.unwrap();
    for event in events {
        repos
            .event
            .delete(forge_run::domain::value_objects::ids::EventId(event.id))
            .await
            .unwrap();
    }
    repos.job.delete(job.id).await.unwrap();
}

#[tokio::test]
async fn given_active_lease_when_execute_should_not_requeue_job() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle));

    let mut job = Job::new_instant(
        JobId::new(),
        forge_run::domain::value_objects::ids::ClientId::new(),
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    job.state = JobState::Assigned;
    job.lease_owner = Some("worker-1".to_string());
    job.lease_expires_at = Some(Timestamp::from(
        Timestamp::now_utc().as_inner() + time::Duration::seconds(30),
    ));
    job.updated_at = Timestamp::now_utc();
    repos.job.insert(&job).await.unwrap();

    let result = RequeueExpiredLeasesUseCase::execute(&ctx, Timestamp::now_utc(), 10)
        .await
        .unwrap();

    assert!(result.jobs.is_empty());

    repos.job.delete(job.id).await.unwrap();
}
