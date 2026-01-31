use std::sync::Arc;

use crate::infrastructure::db::database::{Database, DatabaseError};
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::postgres::api_key_store_postgres::ApiKeyStorePostgres;
use crate::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
use crate::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
use crate::infrastructure::db::postgres::idempotency_key_store_postgres::IdempotencyKeyStorePostgres;
use crate::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
use crate::infrastructure::db::postgres::report_store_postgres::ReportStorePostgres;
use crate::infrastructure::db::repositories::api_key_repository::ApiKeyRepository;
use crate::infrastructure::db::repositories::client_repository::ClientRepository;
use crate::infrastructure::db::repositories::event_repository::EventRepository;
use crate::infrastructure::db::repositories::idempotency_key_repository::IdempotencyKeyRepository;
use crate::infrastructure::db::repositories::job_repository::JobRepository;
use crate::infrastructure::db::repositories::report_repository::ReportRepository;
use crate::infrastructure::db::repositories::webhook_repository::WebhookRepository;
use crate::infrastructure::db::stores::webhook_store::DisabledWebhookStore;
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
pub struct Repositories {
    pub tx: Option<Arc<PostgresDatabase>>,
    pub job: Arc<JobRepository>,
    pub event: Arc<EventRepository>,
    pub report: Arc<ReportRepository>,
    pub client: Arc<ClientRepository>,
    pub api_key: Arc<ApiKeyRepository>,
    pub idempotency: Arc<IdempotencyKeyRepository>,
    pub webhook: Arc<WebhookRepository>,
}

impl Repositories {
    /// Build all repositories backed by Postgres stores.
    pub fn postgres(db: Arc<PostgresDatabase>) -> Self {
        let job_store = Arc::new(JobStorePostgres::new(db.clone()));
        let event_store = Arc::new(EventStorePostgres::new(db.clone()));
        let report_store = Arc::new(ReportStorePostgres::new(db.clone()));
        let client_store = Arc::new(ClientStorePostgres::new(db.clone()));
        let api_key_store = Arc::new(ApiKeyStorePostgres::new(db.clone()));
        let id_store = Arc::new(IdempotencyKeyStorePostgres::new(db.clone()));
        let webhook_store = Arc::new(DisabledWebhookStore);

        Self {
            tx: Some(db.clone()),
            job: Arc::new(JobRepository::new(job_store)),
            event: Arc::new(EventRepository::new(event_store)),
            report: Arc::new(ReportRepository::new(report_store)),
            client: Arc::new(ClientRepository::new(client_store)),
            api_key: Arc::new(ApiKeyRepository::new(api_key_store)),
            idempotency: Arc::new(IdempotencyKeyRepository::new(id_store)),
            webhook: Arc::new(WebhookRepository::new(webhook_store)),
        }
    }

    /// Run multiple repository operations inside a single transaction.
    pub async fn with_tx<T, F>(&self, f: F) -> Result<T, DatabaseError>
    where
        for<'c> F: FnOnce(
            &'c mut sqlx::Transaction<'_, sqlx::Postgres>,
        )
            -> Pin<Box<dyn Future<Output = Result<T, DatabaseError>> + Send + 'c>>,
    {
        let Some(db) = self.tx.as_ref() else {
            return Err(DatabaseError::Connection("tx_unavailable".to_string()));
        };
        db.with_tx(f).await
    }

    /// Execute a raw SQL statement outside a transaction.
    pub async fn execute(&self, query: &str) -> Result<u64, DatabaseError> {
        let Some(db) = self.tx.as_ref() else {
            return Err(DatabaseError::Connection("db_unavailable".to_string()));
        };
        db.execute(query).await
    }
}
