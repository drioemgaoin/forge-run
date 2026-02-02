use crate::infrastructure::db::dto::{WebhookDeliveryRow, WebhookDeliveryStats};
use crate::infrastructure::db::stores::webhook_delivery_store::{
    WebhookDeliveryRepositoryError, WebhookDeliveryStore,
};
use std::sync::Arc;
use time::OffsetDateTime;

pub struct WebhookDeliveryRepository {
    store: Arc<dyn WebhookDeliveryStore>,
}

impl WebhookDeliveryRepository {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<dyn WebhookDeliveryStore>) -> Self {
        Self { store }
    }

    /// Fetch a delivery by its ID. Returns `None` if it doesn't exist.
    pub async fn get(
        &self,
        delivery_id: uuid::Uuid,
    ) -> Result<Option<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        self.store
            .get(delivery_id)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    /// List deliveries that are due for processing.
    pub async fn list_due(
        &self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        self.store
            .list_due(now, limit)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    /// Create a delivery and return what was stored in the database.
    pub async fn insert(
        &self,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        self.store
            .insert(row)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    /// Create a delivery inside an existing transaction.
    pub async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        self.store
            .insert_tx(tx, row)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    /// Update a delivery and return what was stored in the database.
    pub async fn update(
        &self,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        self.store
            .update(row)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    /// Delete a delivery by its ID. Returns an error if it doesn't exist.
    pub async fn delete(
        &self,
        delivery_id: uuid::Uuid,
    ) -> Result<(), WebhookDeliveryRepositoryError> {
        self.store
            .delete(delivery_id)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }

    /// Return aggregate delivery counts by status.
    pub async fn stats(&self) -> Result<WebhookDeliveryStats, WebhookDeliveryRepositoryError> {
        self.store
            .stats()
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)
    }
}
