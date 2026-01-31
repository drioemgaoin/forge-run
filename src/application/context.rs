use std::sync::Arc;

use crate::domain::services::job_lifecycle::JobLifecycleService;
use crate::infrastructure::db::repositories::Repositories;

/// Shared application resources used by use cases and services.
pub struct AppContext {
    pub repos: Repositories,
    pub job_lifecycle: Arc<dyn JobLifecycleService>,
}

impl AppContext {
    /// Build a new application context with shared repositories and services.
    pub fn new(repos: Repositories, job_lifecycle: Arc<dyn JobLifecycleService>) -> Self {
        Self {
            repos,
            job_lifecycle,
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use super::AppContext;
    use crate::domain::entities::event::Event;
    use crate::domain::entities::job::{Job, JobOutcome, JobState};
    use crate::domain::entities::report::{EventSnapshot, Report};
    use crate::domain::services::job_lifecycle::{JobLifecycleError, JobLifecycleService};
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::{
        ApiKeyRow, ClientRow, EventRow, IdempotencyKeyRow, JobRow, ReportRow, WebhookRow,
    };
    use crate::infrastructure::db::repositories::Repositories;
    use crate::infrastructure::db::repositories::api_key_repository::ApiKeyRepository;
    use crate::infrastructure::db::repositories::client_repository::ClientRepository;
    use crate::infrastructure::db::repositories::event_repository::EventRepository;
    use crate::infrastructure::db::repositories::idempotency_key_repository::IdempotencyKeyRepository;
    use crate::infrastructure::db::repositories::job_repository::JobRepository;
    use crate::infrastructure::db::repositories::report_repository::ReportRepository;
    use crate::infrastructure::db::repositories::webhook_repository::WebhookRepository;
    use crate::infrastructure::db::stores::api_key_store::{ApiKeyRepositoryError, ApiKeyStore};
    use crate::infrastructure::db::stores::client_store::{ClientRepositoryError, ClientStore};
    use crate::infrastructure::db::stores::event_store::{EventRepositoryError, EventStore};
    use crate::infrastructure::db::stores::idempotency_key_store::{
        IdempotencyKeyRepositoryError, IdempotencyKeyStore,
    };
    use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
    use crate::infrastructure::db::stores::report_store::{ReportRepositoryError, ReportStore};
    use crate::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};
    use async_trait::async_trait;
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct NullJobStore;

    #[async_trait]
    impl JobStore for NullJobStore {
        async fn get(&self, _job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn insert(&self, _row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn update(&self, _row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn delete(&self, _job_id: uuid::Uuid) -> Result<(), JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn list_due_deferred(
            &self,
            _now: time::OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn claim_next_queued(&self) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &JobRow,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &JobRow,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<(), JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }

        async fn claim_next_queued_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullEventStore;

    #[async_trait]
    impl EventStore for NullEventStore {
        async fn get(&self, _id: uuid::Uuid) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn insert(&self, _row: &EventRow) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn update(&self, _row: &EventRow) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn delete(&self, _id: uuid::Uuid) -> Result<(), EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn list_by_job_id(
            &self,
            _job_id: uuid::Uuid,
        ) -> Result<Vec<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn get_by_job_id_and_name(
            &self,
            _job_id: uuid::Uuid,
            _event_name: &str,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &EventRow,
        ) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &EventRow,
        ) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _id: uuid::Uuid,
        ) -> Result<(), EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn list_by_job_id_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Vec<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _event_id: uuid::Uuid,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }

        async fn get_by_job_id_and_name_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
            _event_name: &str,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullReportStore;

    #[async_trait]
    impl ReportStore for NullReportStore {
        async fn get(
            &self,
            _job_id: uuid::Uuid,
        ) -> Result<Option<ReportRow>, ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn insert(&self, _row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn update(&self, _row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn delete(&self, _job_id: uuid::Uuid) -> Result<(), ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ReportRow,
        ) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ReportRow,
        ) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<(), ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Option<ReportRow>, ReportRepositoryError> {
            Err(ReportRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullClientStore;

    #[async_trait]
    impl ClientStore for NullClientStore {
        async fn get(
            &self,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ClientRow>, ClientRepositoryError> {
            Err(ClientRepositoryError::StorageUnavailable)
        }

        async fn insert(&self, _row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
            Err(ClientRepositoryError::StorageUnavailable)
        }

        async fn update(&self, _row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
            Err(ClientRepositoryError::StorageUnavailable)
        }

        async fn delete(&self, _client_id: uuid::Uuid) -> Result<(), ClientRepositoryError> {
            Err(ClientRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullApiKeyStore;

    #[async_trait]
    impl ApiKeyStore for NullApiKeyStore {
        async fn get(
            &self,
            _key_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn insert(&self, _row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn update(&self, _row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn delete(&self, _key_id: uuid::Uuid) -> Result<(), ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn get_active_by_client_id(
            &self,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn get_active_by_prefix_and_hash(
            &self,
            _key_prefix: &str,
            _key_hash: &str,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ApiKeyRow,
        ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ApiKeyRow,
        ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _key_id: uuid::Uuid,
        ) -> Result<(), ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _key_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn get_active_by_client_id_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }

        async fn get_active_by_prefix_and_hash_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _key_prefix: &str,
            _key_hash: &str,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullIdempotencyStore;

    #[async_trait]
    impl IdempotencyKeyStore for NullIdempotencyStore {
        async fn get(
            &self,
            _client_id: uuid::Uuid,
            _key: &str,
        ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::StorageUnavailable)
        }

        async fn insert(
            &self,
            _row: &IdempotencyKeyRow,
        ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::StorageUnavailable)
        }

        async fn delete(
            &self,
            _client_id: uuid::Uuid,
            _key: &str,
        ) -> Result<(), IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::StorageUnavailable)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _client_id: uuid::Uuid,
            _key: &str,
        ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::StorageUnavailable)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &IdempotencyKeyRow,
        ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::StorageUnavailable)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _client_id: uuid::Uuid,
            _key: &str,
        ) -> Result<(), IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullWebhookStore;

    #[async_trait]
    impl WebhookStore for NullWebhookStore {
        async fn get(&self, _id: uuid::Uuid) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
            Err(WebhookRepositoryError::StorageUnavailable)
        }

        async fn insert(&self, _row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError> {
            Err(WebhookRepositoryError::StorageUnavailable)
        }

        async fn delete(&self, _id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
            Err(WebhookRepositoryError::StorageUnavailable)
        }
    }

    #[derive(Clone)]
    pub struct NullLifecycle;

    #[async_trait]
    impl JobLifecycleService for NullLifecycle {
        async fn create_instant(
            &self,
            _job_id: JobId,
            _client_id: ClientId,
            _callback_url: Option<String>,
            _work_kind: Option<String>,
        ) -> Result<(Job, Event), JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }

        async fn create_deferred(
            &self,
            _job_id: JobId,
            _client_id: ClientId,
            _execution_at: Timestamp,
            _callback_url: Option<String>,
            _work_kind: Option<String>,
        ) -> Result<(Job, Event), JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }

        async fn transition(
            &self,
            _job: &mut Job,
            _next_state: JobState,
        ) -> Result<Event, JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }

        async fn finalize_report(
            &self,
            _job_id: JobId,
            _events: Vec<EventSnapshot>,
            _outcome: JobOutcome,
            _outcome_reason: Option<String>,
        ) -> Result<Report, JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }
    }

    pub fn test_context() -> AppContext {
        // Step 1: Build repositories backed by null stores (tests override as needed).
        let repos = Repositories {
            tx: None,
            job: Arc::new(JobRepository::new(Arc::new(NullJobStore))),
            event: Arc::new(EventRepository::new(Arc::new(NullEventStore))),
            report: Arc::new(ReportRepository::new(Arc::new(NullReportStore))),
            client: Arc::new(ClientRepository::new(Arc::new(NullClientStore))),
            api_key: Arc::new(ApiKeyRepository::new(Arc::new(NullApiKeyStore))),
            idempotency: Arc::new(IdempotencyKeyRepository::new(Arc::new(
                NullIdempotencyStore,
            ))),
            webhook: Arc::new(WebhookRepository::new(Arc::new(NullWebhookStore))),
        };
        // Step 2: Return a context with a no-op lifecycle service.
        AppContext {
            repos,
            job_lifecycle: Arc::new(NullLifecycle),
        }
    }
}
