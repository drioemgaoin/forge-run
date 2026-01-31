use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ApiKeyRow;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for ApiKeyRepositoryError {
    fn from(_: DatabaseError) -> Self {
        ApiKeyRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait ApiKeyStore: Send + Sync {
    /// Fetch an API key by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, key_id: uuid::Uuid) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError>;
    /// Create an API key and return exactly what was stored in the database.
    async fn insert(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError>;
    /// Update an API key and return exactly what was stored in the database.
    async fn update(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError>;
    /// Delete an API key by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, key_id: uuid::Uuid) -> Result<(), ApiKeyRepositoryError>;
    /// Fetch the active (non-revoked) API key for a client.
    async fn get_active_by_client_id(
        &self,
        client_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError>;
    /// Fetch an active API key by prefix+hash (used for auth).
    async fn get_active_by_prefix_and_hash(
        &self,
        key_prefix: &str,
        key_hash: &str,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError>;

    /// Create an API key inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ApiKeyRow,
    ) -> Result<ApiKeyRow, ApiKeyRepositoryError>;
    /// Update an API key inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ApiKeyRow,
    ) -> Result<ApiKeyRow, ApiKeyRepositoryError>;
    /// Delete an API key inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        key_id: uuid::Uuid,
    ) -> Result<(), ApiKeyRepositoryError>;
    /// Fetch an API key by ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        key_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError>;
    /// Fetch the active (non-revoked) API key for a client inside an existing transaction.
    async fn get_active_by_client_id_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        client_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError>;
    /// Fetch an active API key by prefix+hash inside an existing transaction.
    async fn get_active_by_prefix_and_hash_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        key_prefix: &str,
        key_hash: &str,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::ApiKeyRepositoryError;
    use crate::infrastructure::db::database::DatabaseError;

    #[test]
    fn given_database_error_when_converted_should_map_to_storage_unavailable() {
        let err = ApiKeyRepositoryError::from(DatabaseError::Query("boom".to_string()));
        assert_eq!(err, ApiKeyRepositoryError::StorageUnavailable);
    }
}
