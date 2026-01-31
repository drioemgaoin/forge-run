use forge_run::domain::entities::job::{Job, JobState};
use forge_run::domain::value_objects::ids::{ClientId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
use forge_run::infrastructure::db::repositories::job_repository::JobRepository;
use std::sync::Arc;
use time::OffsetDateTime;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn setup_repo() -> Option<JobRepository> {
    let url = test_db_url()?;
    let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.ok()?);
    let store = JobStorePostgres::new(db);
    Some(JobRepository::new(Arc::new(store)))
}

fn sample_instant_job() -> Job {
    Job::new_instant(
        JobId::new(),
        ClientId::new(),
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap()
}

fn sample_deferred_job() -> Job {
    let now = Timestamp::now_utc();
    let execution_at = Timestamp::from(now.as_inner() + time::Duration::seconds(1));
    Job::new_deferred(
        JobId::new(),
        ClientId::new(),
        execution_at,
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap()
}

#[tokio::test]
async fn given_job_when_insert_should_return_stored_job() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let job = sample_instant_job();

    let stored = repo.insert(&job).await.unwrap();

    assert_eq!(stored.id, job.id);
    assert_eq!(stored.client_id, job.client_id);
    assert_eq!(stored.job_type, job.job_type);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_existing_job_when_get_should_return_job() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let job = sample_instant_job();
    let stored = repo.insert(&job).await.unwrap();

    let fetched = repo.get(stored.id).await.unwrap();

    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().id, stored.id);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_existing_job_when_update_should_return_updated_job() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut job = sample_instant_job();
    let stored = repo.insert(&job).await.unwrap();
    job.state = JobState::Queued;
    job.updated_at = Timestamp::now_utc();

    let updated = repo.update(&job).await.unwrap();

    assert_eq!(updated.id, stored.id);
    assert_eq!(updated.state, JobState::Queued);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_deferred_job_due_when_list_due_deferred_should_return_job() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut job = sample_deferred_job();
    job.executed_at = Some(Timestamp::from(
        OffsetDateTime::now_utc() - time::Duration::seconds(1),
    ));
    let stored = repo.insert(&job).await.unwrap();

    let jobs = repo
        .list_due_deferred(Timestamp::from(OffsetDateTime::now_utc()), 10)
        .await
        .unwrap();

    assert!(jobs.iter().any(|j| j.id == stored.id));
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_queued_job_when_claim_next_queued_should_assign_job() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut job = sample_instant_job();
    job.state = JobState::Queued;
    job.updated_at = Timestamp::now_utc();
    let stored = repo.insert(&job).await.unwrap();

    let claimed = repo.claim_next_queued().await.unwrap();

    assert!(claimed.is_some());
    let claimed = claimed.unwrap();
    assert_eq!(claimed.id, stored.id);
    assert_eq!(claimed.state, JobState::Assigned);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_missing_job_when_get_should_return_none() {
    let Some(repo) = setup_repo().await else {
        return;
    };

    let fetched = repo.get(JobId::new()).await.unwrap();

    assert!(fetched.is_none());
}
