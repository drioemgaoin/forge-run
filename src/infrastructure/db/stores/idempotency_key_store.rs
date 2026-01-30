use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::IdempotencyKeyRow;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyKeyRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for IdempotencyKeyRepositoryError {
    fn from(_: DatabaseError) -> Self {
        IdempotencyKeyRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait IdempotencyKeyStore: Send + Sync {
    /// Fetch an idempotency entry by client + key. Returns `None` if it doesn't exist.
    async fn get(
        &self,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError>;
    /// Create an idempotency entry and return exactly what was stored in the database.
    async fn insert(
        &self,
        row: &IdempotencyKeyRow,
    ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError>;
    /// Delete an idempotency entry. Returns an error if it doesn't exist.
    async fn delete(
        &self,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<(), IdempotencyKeyRepositoryError>;

    /// Create an idempotency entry inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &IdempotencyKeyRow,
    ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError>;
    /// Delete an idempotency entry inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<(), IdempotencyKeyRepositoryError>;
    /// Fetch an idempotency entry by client + key inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::IdempotencyKeyRepositoryError;
    use crate::infrastructure::db::database::DatabaseError;

    #[test]
    fn given_database_error_when_converted_should_map_to_storage_unavailable() {
        let err = IdempotencyKeyRepositoryError::from(DatabaseError::Query("boom".to_string()));
        assert_eq!(err, IdempotencyKeyRepositoryError::StorageUnavailable);
    }
}
