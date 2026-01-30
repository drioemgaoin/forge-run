use forge_run::application::usecases::cleanup_retention::CleanupRetentionUseCase;
use forge_run::domain::entities::job::{Job, JobOutcome, JobState};
use forge_run::domain::value_objects::ids::{ClientId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::dto::{ApiKeyRow, ClientRow, EventRow, JobRow, ReportRow};
use forge_run::infrastructure::db::postgres::api_key_store_postgres::ApiKeyStorePostgres;
use forge_run::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
use forge_run::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
use forge_run::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
use forge_run::infrastructure::db::postgres::report_store_postgres::ReportStorePostgres;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::stores::api_key_store::ApiKeyStore;
use forge_run::infrastructure::db::stores::client_store::ClientStore;
use forge_run::infrastructure::db::stores::event_store::EventStore;
use forge_run::infrastructure::db::stores::job_store::JobStore;
use forge_run::infrastructure::db::stores::report_store::ReportStore;
use std::sync::Arc;
use time::OffsetDateTime;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn given_expired_job_and_revoked_key_when_cleanup_should_delete_both() {
    let Some(url) = test_db_url() else { return; };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let usecase = CleanupRetentionUseCase { db: db.clone() };

    let client_store = ClientStorePostgres::new(db.clone());
    let job_store = JobStorePostgres::new(db.clone());
    let event_store = EventStorePostgres::new(db.clone());
    let report_store = ReportStorePostgres::new(db.clone());
    let api_key_store = ApiKeyStorePostgres::new(db.clone());

    let client_id = ClientId::new();
    let client_row = ClientRow {
        id: client_id.0,
        created_at: Timestamp::now_utc().as_inner(),
    };
    client_store.insert(&client_row).await.unwrap();

    let mut job = Job::new_instant(
        JobId::new(),
        client_id,
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    job.state = JobState::Succeeded;
    job.outcome = Some(JobOutcome::Success);
    job.updated_at = Timestamp::from(OffsetDateTime::now_utc() - time::Duration::days(31));
    let job_row = JobRow::from_job(&job);
    job_store.insert(&job_row).await.unwrap();

    let event = EventRow {
        id: uuid::Uuid::new_v4(),
        job_id: job.id.0,
        event_name: "job_succeeded".to_string(),
        prev_state: "running".to_string(),
        next_state: "succeeded".to_string(),
        occurred_at: OffsetDateTime::now_utc(),
    };
    event_store.insert(&event).await.unwrap();

    let report = ReportRow {
        job_id: job.id.0,
        outcome: "success".to_string(),
        outcome_reason: None,
        started_at: OffsetDateTime::now_utc(),
        finished_at: OffsetDateTime::now_utc(),
        duration_ms: 100,
        created_at: OffsetDateTime::now_utc(),
    };
    report_store.insert(&report).await.unwrap();

    let revoked_at = OffsetDateTime::now_utc() - time::Duration::days(91);
    let api_key = ApiKeyRow {
        id: uuid::Uuid::new_v4(),
        client_id: client_id.0,
        key_hash: "hash".to_string(),
        key_prefix: "pref".to_string(),
        created_at: OffsetDateTime::now_utc(),
        expires_at: None,
        revoked_at: Some(revoked_at),
    };
    api_key_store.insert(&api_key).await.unwrap();

    let result = usecase.execute().await.unwrap();

    assert!(result.jobs_deleted >= 1);
    assert!(result.api_keys_deleted >= 1);

    let job_after = job_store.get(job.id.0).await.unwrap();
    assert!(job_after.is_none());

    let events_after = event_store.list_by_job_id(job.id.0).await.unwrap();
    assert!(events_after.is_empty());

    let report_after = report_store.get(job.id.0).await.unwrap();
    assert!(report_after.is_none());

    let api_after = api_key_store.get(api_key.id).await.unwrap();
    assert!(api_after.is_none());

    client_store.delete(client_id.0).await.unwrap();
}
