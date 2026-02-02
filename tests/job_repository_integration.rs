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

    let claimed = repo
        .claim_next_queued("worker-1", time::Duration::seconds(30))
        .await
        .unwrap();

    assert!(claimed.is_some());
    let claimed = claimed.unwrap();
    assert_eq!(claimed.id, stored.id);
    assert_eq!(claimed.state, JobState::Assigned);
    assert_eq!(claimed.lease_owner.as_deref(), Some("worker-1"));
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_multiple_queued_jobs_when_claim_next_should_return_oldest() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut first = sample_instant_job();
    first.state = JobState::Queued;
    first.created_at = Timestamp::from(OffsetDateTime::now_utc() - time::Duration::seconds(10));
    first.updated_at = first.created_at;
    let stored_first = repo.insert(&first).await.unwrap();

    let mut second = sample_instant_job();
    second.state = JobState::Queued;
    second.updated_at = Timestamp::now_utc();
    let stored_second = repo.insert(&second).await.unwrap();

    let claimed = repo
        .claim_next_queued("worker-2", time::Duration::seconds(30))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(claimed.id, stored_first.id);

    repo.delete(stored_first.id).await.unwrap();
    repo.delete(stored_second.id).await.unwrap();
}

#[tokio::test]
async fn given_missing_job_when_get_should_return_none() {
    let Some(repo) = setup_repo().await else {
        return;
    };

    let fetched = repo.get(JobId::new()).await.unwrap();

    assert!(fetched.is_none());
}

#[tokio::test]
async fn given_assigned_job_when_heartbeat_should_extend_lease() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut job = sample_instant_job();
    job.state = JobState::Assigned;
    job.lease_owner = Some("worker-1".to_string());
    job.lease_expires_at = Some(Timestamp::from(
        OffsetDateTime::now_utc() + time::Duration::seconds(5),
    ));
    job.updated_at = Timestamp::now_utc();
    let stored = repo.insert(&job).await.unwrap();

    let updated = repo
        .heartbeat(stored.id, "worker-1", time::Duration::seconds(30))
        .await
        .unwrap();

    assert!(updated.lease_expires_at.is_some());
    assert!(updated.heartbeat_at.is_some());
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_assigned_job_with_wrong_worker_when_heartbeat_should_return_error() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut job = sample_instant_job();
    job.state = JobState::Assigned;
    job.lease_owner = Some("worker-1".to_string());
    job.lease_expires_at = Some(Timestamp::from(
        OffsetDateTime::now_utc() + time::Duration::seconds(5),
    ));
    job.updated_at = Timestamp::now_utc();
    let stored = repo.insert(&job).await.unwrap();

    let err = repo
        .heartbeat(stored.id, "worker-2", time::Duration::seconds(30))
        .await
        .unwrap_err();

    assert_eq!(
        err,
        forge_run::infrastructure::db::stores::job_store::JobRepositoryError::NotFound
    );

    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_queued_jobs_when_queue_depth_should_count_only_due() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut due = sample_instant_job();
    due.state = JobState::Queued;
    due.updated_at = Timestamp::now_utc();
    let stored_due = repo.insert(&due).await.unwrap();

    let mut future = sample_instant_job();
    future.state = JobState::Queued;
    future.executed_at = Some(Timestamp::from(
        OffsetDateTime::now_utc() + time::Duration::seconds(60),
    ));
    future.updated_at = Timestamp::now_utc();
    let stored_future = repo.insert(&future).await.unwrap();

    let depth = repo.queue_depth().await.unwrap();

    assert_eq!(depth, 1);

    repo.delete(stored_due.id).await.unwrap();
    repo.delete(stored_future.id).await.unwrap();
}
