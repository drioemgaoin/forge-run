use forge_run::domain::entities::client::Client;
use forge_run::domain::entities::event::Event;
use forge_run::domain::entities::job::{Job, JobState};
use forge_run::domain::value_objects::ids::{ClientId, EventId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::database::DatabaseError;
use forge_run::infrastructure::db::dto::{
    ApiKeyRow, ClientRow, EventRow, IdempotencyKeyRow, JobRow,
};
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::postgres::api_key_store_postgres::ApiKeyStorePostgres;
use forge_run::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
use forge_run::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
use forge_run::infrastructure::db::postgres::idempotency_key_store_postgres::IdempotencyKeyStorePostgres;
use forge_run::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
use forge_run::infrastructure::db::stores::api_key_store::ApiKeyStore;
use forge_run::infrastructure::db::stores::client_store::ClientStore;
use forge_run::infrastructure::db::stores::event_store::EventStore;
use forge_run::infrastructure::db::stores::idempotency_key_store::IdempotencyKeyStore;
use forge_run::infrastructure::db::stores::job_store::JobStore;
use std::sync::Arc;
use time::OffsetDateTime;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn sample_job(client_id: ClientId) -> Job {
    Job::new_instant(
        JobId::new(),
        client_id,
        None,
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap()
}

fn sample_client() -> Client {
    Client::new(ClientId::new())
}

fn sample_api_key(client_id: ClientId, now: OffsetDateTime) -> ApiKeyRow {
    ApiKeyRow {
        id: uuid::Uuid::new_v4(),
        client_id: client_id.0,
        key_hash: "hash".to_string(),
        key_prefix: "pref".to_string(),
        created_at: now,
        expires_at: None,
        revoked_at: None,
    }
}

#[tokio::test]
async fn given_failed_event_insert_when_creating_job_should_rollback_job_insert() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let client_store = ClientStorePostgres::new(db.clone());
    let job_store = JobStorePostgres::new(db.clone());
    let event_store = EventStorePostgres::new(db.clone());

    let client = sample_client();
    let client_row = ClientRow::from_client(&client);
    client_store.insert(&client_row).await.unwrap();

    let job = sample_job(client.id);
    let job_row = JobRow::from_job(&job);
    let mut event_row = EventRow::from_event(&Event::new_created(
        EventId::new(),
        job.id,
        Timestamp::now_utc(),
    ));
    event_row.job_id = uuid::Uuid::new_v4();

    let job_store_tx = job_store.clone();
    let event_store_tx = event_store.clone();
    let result = db
        .with_tx(|tx| {
            let job_row = job_row.clone();
            let event_row = event_row.clone();
            Box::pin(async move {
                job_store_tx
                    .insert_tx(tx, &job_row)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                event_store_tx
                    .insert_tx(tx, &event_row)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err());

    let stored = job_store.get(job.id.0).await.unwrap();
    assert!(stored.is_none());

    client_store.delete(client.id.0).await.unwrap();
}

#[tokio::test]
async fn given_failed_event_insert_when_updating_job_should_rollback_job_update() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let client_store = ClientStorePostgres::new(db.clone());
    let job_store = JobStorePostgres::new(db.clone());
    let event_store = EventStorePostgres::new(db.clone());

    let client = sample_client();
    let client_row = ClientRow::from_client(&client);
    client_store.insert(&client_row).await.unwrap();

    let mut job = sample_job(client.id);
    let job_row = JobRow::from_job(&job);
    let stored = job_store.insert(&job_row).await.unwrap();

    job.state = JobState::Queued;
    job.updated_at = Timestamp::now_utc();
    let updated_row = JobRow::from_job(&job);

    let mut event_row = EventRow::from_event(
        &Event::from_transition(EventId::new(), job.id, JobState::Created, JobState::Queued)
            .unwrap(),
    );
    event_row.job_id = uuid::Uuid::new_v4();

    let job_store_tx = job_store.clone();
    let event_store_tx = event_store.clone();
    let result = db
        .with_tx(|tx| {
            let updated_row = updated_row.clone();
            let event_row = event_row.clone();
            Box::pin(async move {
                job_store_tx
                    .update_tx(tx, &updated_row)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                event_store_tx
                    .insert_tx(tx, &event_row)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err());

    let fetched = job_store.get(stored.id).await.unwrap().unwrap();
    assert_eq!(fetched.state, JobState::Created.as_str());

    job_store.delete(stored.id).await.unwrap();
    client_store.delete(client.id.0).await.unwrap();
}

#[tokio::test]
async fn given_failed_api_key_insert_when_revoking_should_rollback_update() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let client_store = ClientStorePostgres::new(db.clone());
    let api_key_store = ApiKeyStorePostgres::new(db.clone());

    let client = sample_client();
    let client_row = ClientRow::from_client(&client);
    client_store.insert(&client_row).await.unwrap();

    let now = OffsetDateTime::now_utc();
    let key_row = sample_api_key(client.id, now);
    let stored = api_key_store.insert(&key_row).await.unwrap();

    let mut revoked = stored.clone();
    revoked.revoked_at = Some(OffsetDateTime::now_utc());

    let mut invalid = sample_api_key(client.id, now);
    invalid.client_id = uuid::Uuid::new_v4();

    let api_key_store_tx = api_key_store.clone();
    let result = db
        .with_tx(|tx| {
            let revoked = revoked.clone();
            let invalid = invalid.clone();
            Box::pin(async move {
                api_key_store_tx
                    .update_tx(tx, &revoked)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                api_key_store_tx
                    .insert_tx(tx, &invalid)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err());

    let fetched = api_key_store.get(stored.id).await.unwrap().unwrap();
    assert_eq!(fetched.revoked_at, None);

    api_key_store.delete(stored.id).await.unwrap();
    client_store.delete(client.id.0).await.unwrap();
}

#[tokio::test]
async fn given_failed_idempotency_insert_when_creating_job_should_rollback_job_insert() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let client_store = ClientStorePostgres::new(db.clone());
    let job_store = JobStorePostgres::new(db.clone());
    let idempotency_store = IdempotencyKeyStorePostgres::new(db.clone());

    let client = sample_client();
    let client_row = ClientRow::from_client(&client);
    client_store.insert(&client_row).await.unwrap();

    let job = sample_job(client.id);
    let job_row = JobRow::from_job(&job);
    let id_row = IdempotencyKeyRow {
        client_id: client.id.0,
        idempotency_key: "idem-key".to_string(),
        job_id: Some(uuid::Uuid::new_v4()),
        created_at: OffsetDateTime::now_utc(),
    };

    let job_store_tx = job_store.clone();
    let id_store_tx = idempotency_store.clone();
    let result = db
        .with_tx(|tx| {
            let job_row = job_row.clone();
            let mut id_row = id_row.clone();
            id_row.job_id = Some(uuid::Uuid::new_v4());
            Box::pin(async move {
                job_store_tx
                    .insert_tx(tx, &job_row)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                id_store_tx
                    .insert_tx(tx, &id_row)
                    .await
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err());

    let stored_job = job_store.get(job.id.0).await.unwrap();
    assert!(stored_job.is_none());

    let stored_key = idempotency_store
        .get(client.id.0, &id_row.idempotency_key)
        .await
        .unwrap();
    assert!(stored_key.is_none());

    client_store.delete(client.id.0).await.unwrap();
}
