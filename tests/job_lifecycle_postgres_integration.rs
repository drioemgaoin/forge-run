use forge_run::domain::services::job_lifecycle::{JobLifecycle, JobLifecycleService};
use forge_run::domain::value_objects::ids::{ClientId, JobId};
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
use forge_run::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
use forge_run::infrastructure::db::postgres::report_store_postgres::ReportStorePostgres;
use forge_run::infrastructure::db::stores::event_store::EventStore;
use forge_run::infrastructure::db::stores::job_store::JobStore;
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
    let job_store = JobStorePostgres::new(db.clone());
    let event_store = EventStorePostgres::new(db.clone());
    let report_store = ReportStorePostgres::new(db.clone());
    let lifecycle = JobLifecycle::new(
        db.clone(),
        job_store.clone(),
        event_store.clone(),
        report_store,
    );

    let (job, event) = lifecycle
        .create_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .await
        .unwrap();

    let stored_job = job_store.get(job.id.0).await.unwrap();
    let stored_event = event_store.get(event.id.0).await.unwrap();

    assert!(stored_job.is_some());
    assert!(stored_event.is_some());

    event_store.delete(event.id.0).await.unwrap();
    job_store.delete(job.id.0).await.unwrap();
}
