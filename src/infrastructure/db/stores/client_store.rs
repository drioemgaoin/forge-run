use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ClientRow;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for ClientRepositoryError {
    fn from(_: DatabaseError) -> Self {
        ClientRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait ClientStore: Send + Sync {
    /// Fetch a client by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, client_id: uuid::Uuid) -> Result<Option<ClientRow>, ClientRepositoryError>;
    /// Create a client and return exactly what was stored in the database.
    async fn insert(&self, row: &ClientRow) -> Result<ClientRow, ClientRepositoryError>;
    /// Update a client and return exactly what was stored in the database.
    async fn update(&self, row: &ClientRow) -> Result<ClientRow, ClientRepositoryError>;
    /// Delete a client by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, client_id: uuid::Uuid) -> Result<(), ClientRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::ClientRepositoryError;
    use crate::infrastructure::db::database::DatabaseError;

    #[test]
    fn given_database_error_when_converted_should_map_to_storage_unavailable() {
        let err = ClientRepositoryError::from(DatabaseError::Query("boom".to_string()));
        assert_eq!(err, ClientRepositoryError::StorageUnavailable);
    }
}
