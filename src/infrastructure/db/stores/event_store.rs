use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::EventRow;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for EventRepositoryError {
    fn from(_: DatabaseError) -> Self {
        EventRepositoryError::StorageUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::EventRepositoryError;
    use crate::infrastructure::db::database::DatabaseError;

    #[test]
    fn given_database_error_when_converted_should_map_to_storage_unavailable() {
        let err = EventRepositoryError::from(DatabaseError::Query("boom".to_string()));
        assert_eq!(err, EventRepositoryError::StorageUnavailable);
    }
}

#[async_trait]
pub trait EventStore: Send + Sync {
    /// Fetch an event by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, event_id: uuid::Uuid) -> Result<Option<EventRow>, EventRepositoryError>;
    /// Create an event and return exactly what was stored in the database.
    async fn insert(&self, row: &EventRow) -> Result<EventRow, EventRepositoryError>;
    /// Update an event and return exactly what was stored in the database.
    async fn update(&self, row: &EventRow) -> Result<EventRow, EventRepositoryError>;
    /// Delete an event by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, event_id: uuid::Uuid) -> Result<(), EventRepositoryError>;
    /// Fetch the first event for a job by name. Returns `None` if it doesn't exist.
    async fn get_by_job_id_and_name(
        &self,
        job_id: uuid::Uuid,
        event_name: &str,
    ) -> Result<Option<EventRow>, EventRepositoryError>;
    /// List all events for a job ordered by time.
    async fn list_by_job_id(
        &self,
        job_id: uuid::Uuid,
    ) -> Result<Vec<EventRow>, EventRepositoryError>;

    /// Create a job inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &EventRow,
    ) -> Result<EventRow, EventRepositoryError>;
    /// Update an event inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &EventRow,
    ) -> Result<EventRow, EventRepositoryError>;
    /// Delete an event inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: uuid::Uuid,
    ) -> Result<(), EventRepositoryError>;
    /// Fetch an event by ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: uuid::Uuid,
    ) -> Result<Option<EventRow>, EventRepositoryError>;
    /// Fetch the first event for a job by name inside an existing transaction.
    async fn get_by_job_id_and_name_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
        event_name: &str,
    ) -> Result<Option<EventRow>, EventRepositoryError>;
    /// List all events for a job ordered by time inside an existing transaction.
    async fn list_by_job_id_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<Vec<EventRow>, EventRepositoryError>;
}
