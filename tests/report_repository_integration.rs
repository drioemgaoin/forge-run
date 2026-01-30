use forge_run::domain::entities::job::{Job, JobOutcome};
use forge_run::domain::entities::report::Report;
use forge_run::domain::value_objects::ids::{ClientId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::dto::JobRow;
use forge_run::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
use forge_run::infrastructure::db::postgres::report_store_postgres::ReportStorePostgres;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::report_repository::ReportRepository;
use forge_run::infrastructure::db::stores::job_store::JobStore;
use std::sync::Arc;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn create_job_id() -> Option<JobId> {
    let url = test_db_url()?;
    let db = PostgresDatabase::connect(&url).await.ok()?;
    let job_store = JobStorePostgres::new(db);
    let job = Job::new_instant(
        JobId::new(),
        ClientId::new(),
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    let row = JobRow::from_job(&job);
    let stored = job_store.insert(&row).await.ok()?;
    Some(JobId(stored.id))
}

async fn setup_repo() -> Option<ReportRepository<ReportStorePostgres>> {
    let url = test_db_url()?;
    let db = PostgresDatabase::connect(&url).await.ok()?;
    let store = ReportStorePostgres::new(db);
    Some(ReportRepository::new(Arc::new(store)))
}

fn sample_report(job_id: JobId) -> Report {
    let now = Timestamp::now_utc();
    Report {
        job_id,
        outcome: JobOutcome::Success,
        outcome_reason: None,
        started_at: now,
        finished_at: now,
        duration_ms: 0,
        events: Vec::new(),
    }
}

#[tokio::test]
async fn given_report_when_insert_should_return_stored_report() {
    let Some(repo) = setup_repo().await else { return; };
    let Some(job_id) = create_job_id().await else { return; };
    let report = sample_report(job_id);

    let stored = repo.insert(&report).await.unwrap();

    assert_eq!(stored.job_id, report.job_id);
    assert_eq!(stored.outcome, report.outcome);
    repo.delete(stored.job_id).await.unwrap();
}

#[tokio::test]
async fn given_existing_report_when_get_should_return_report() {
    let Some(repo) = setup_repo().await else { return; };
    let Some(job_id) = create_job_id().await else { return; };
    let report = sample_report(job_id);
    let stored = repo.insert(&report).await.unwrap();

    let fetched = repo.get(stored.job_id).await.unwrap();

    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().job_id, stored.job_id);
    repo.delete(stored.job_id).await.unwrap();
}

#[tokio::test]
async fn given_existing_report_when_update_should_return_updated_report() {
    let Some(repo) = setup_repo().await else { return; };
    let Some(job_id) = create_job_id().await else { return; };
    let mut report = sample_report(job_id);
    let stored = repo.insert(&report).await.unwrap();
    report.job_id = stored.job_id;
    report.outcome = JobOutcome::Failed;
    report.outcome_reason = Some("boom".to_string());

    let updated = repo.update(&report).await.unwrap();

    assert_eq!(updated.job_id, stored.job_id);
    assert_eq!(updated.outcome, JobOutcome::Failed);
    repo.delete(stored.job_id).await.unwrap();
}

#[tokio::test]
async fn given_missing_report_when_get_should_return_none() {
    let Some(repo) = setup_repo().await else { return; };

    let fetched = repo.get(JobId::new()).await.unwrap();

    assert!(fetched.is_none());
}
