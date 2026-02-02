use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::{WebhookDeliveryRow, WebhookDeliveryStats};
use async_trait::async_trait;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookDeliveryRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for WebhookDeliveryRepositoryError {
    fn from(_: DatabaseError) -> Self {
        WebhookDeliveryRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait WebhookDeliveryStore: Send + Sync {
    /// Fetch a delivery by its ID. Returns `None` if it doesn't exist.
    async fn get(
        &self,
        delivery_id: uuid::Uuid,
    ) -> Result<Option<WebhookDeliveryRow>, WebhookDeliveryRepositoryError>;
    /// List deliveries that are due for processing.
    async fn list_due(
        &self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRow>, WebhookDeliveryRepositoryError>;
    /// Create a delivery and return exactly what was stored in the database.
    async fn insert(
        &self,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError>;
    /// Create a delivery inside an existing transaction.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError>;
    /// Update a delivery and return exactly what was stored in the database.
    async fn update(
        &self,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError>;
    /// Delete a delivery by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, delivery_id: uuid::Uuid) -> Result<(), WebhookDeliveryRepositoryError>;
    /// Return aggregate delivery counts by status.
    async fn stats(&self) -> Result<WebhookDeliveryStats, WebhookDeliveryRepositoryError>;
}

/// A no-op delivery store used when persistence is not configured.
pub struct DisabledWebhookDeliveryStore;

#[async_trait]
impl WebhookDeliveryStore for DisabledWebhookDeliveryStore {
    async fn get(
        &self,
        _delivery_id: uuid::Uuid,
    ) -> Result<Option<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    async fn list_due(
        &self,
        _now: OffsetDateTime,
        _limit: u32,
    ) -> Result<Vec<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    async fn insert(
        &self,
        _row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    async fn insert_tx(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    async fn update(
        &self,
        _row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    async fn delete(&self, _delivery_id: uuid::Uuid) -> Result<(), WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    async fn stats(&self) -> Result<WebhookDeliveryStats, WebhookDeliveryRepositoryError> {
        Err(WebhookDeliveryRepositoryError::StorageUnavailable)
    }
}
