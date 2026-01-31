use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::WebhookRow;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for WebhookRepositoryError {
    fn from(_: DatabaseError) -> Self {
        WebhookRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait WebhookStore: Send + Sync {
    /// Fetch a webhook by its ID. Returns `None` if it doesn't exist.
    async fn get(
        &self,
        webhook_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError>;
    /// Create a webhook and return exactly what was stored in the database.
    async fn insert(&self, row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError>;
    /// Delete a webhook by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError>;
}

/// A no-op webhook store used when persistence is not configured.
pub struct DisabledWebhookStore;

#[async_trait]
impl WebhookStore for DisabledWebhookStore {
    async fn get(
        &self,
        _webhook_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        Err(WebhookRepositoryError::StorageUnavailable)
    }

    async fn insert(&self, _row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError> {
        Err(WebhookRepositoryError::StorageUnavailable)
    }

    async fn delete(&self, _webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
        Err(WebhookRepositoryError::StorageUnavailable)
    }
}
