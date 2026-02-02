use crate::infrastructure::db::dto::WebhookRow;
use crate::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};
use std::sync::Arc;

pub struct WebhookRepository {
    store: Arc<dyn WebhookStore>,
}

impl WebhookRepository {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<dyn WebhookStore>) -> Self {
        Self { store }
    }

    /// Fetch a webhook by its ID. Returns `None` if it doesn't exist.
    pub async fn get(
        &self,
        webhook_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        match self.store.get(webhook_id).await {
            Ok(row) => Ok(row),
            Err(WebhookRepositoryError::NotFound) => Err(WebhookRepositoryError::NotFound),
            Err(WebhookRepositoryError::Conflict) => Err(WebhookRepositoryError::Conflict),
            Err(WebhookRepositoryError::InvalidInput) => Err(WebhookRepositoryError::InvalidInput),
            Err(_) => Err(WebhookRepositoryError::StorageUnavailable),
        }
    }

    /// Fetch the default webhook for a client, if any.
    pub async fn get_default_for_client(
        &self,
        client_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        match self.store.get_default_for_client(client_id).await {
            Ok(row) => Ok(row),
            Err(WebhookRepositoryError::NotFound) => Err(WebhookRepositoryError::NotFound),
            Err(WebhookRepositoryError::Conflict) => Err(WebhookRepositoryError::Conflict),
            Err(WebhookRepositoryError::InvalidInput) => Err(WebhookRepositoryError::InvalidInput),
            Err(_) => Err(WebhookRepositoryError::StorageUnavailable),
        }
    }

    /// Create a webhook and return what was actually stored in the database.
    pub async fn insert(&self, row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError> {
        match self.store.insert(row).await {
            Ok(stored) => Ok(stored),
            Err(WebhookRepositoryError::NotFound) => Err(WebhookRepositoryError::NotFound),
            Err(WebhookRepositoryError::Conflict) => Err(WebhookRepositoryError::Conflict),
            Err(WebhookRepositoryError::InvalidInput) => Err(WebhookRepositoryError::InvalidInput),
            Err(_) => Err(WebhookRepositoryError::StorageUnavailable),
        }
    }

    /// Delete a webhook by its ID. Returns an error if it doesn't exist.
    pub async fn delete(&self, webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
        match self.store.delete(webhook_id).await {
            Ok(()) => Ok(()),
            Err(WebhookRepositoryError::NotFound) => Err(WebhookRepositoryError::NotFound),
            Err(WebhookRepositoryError::Conflict) => Err(WebhookRepositoryError::Conflict),
            Err(WebhookRepositoryError::InvalidInput) => Err(WebhookRepositoryError::InvalidInput),
            Err(_) => Err(WebhookRepositoryError::StorageUnavailable),
        }
    }
}
