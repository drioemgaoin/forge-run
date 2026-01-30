use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ReportRow;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for ReportRepositoryError {
    fn from(_: DatabaseError) -> Self {
        ReportRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait ReportStore: Send + Sync {
    /// Fetch a report by job ID. Returns `None` if it doesn't exist.
    async fn get(&self, job_id: uuid::Uuid) -> Result<Option<ReportRow>, ReportRepositoryError>;
    /// Create a report and return exactly what was stored in the database.
    async fn insert(&self, row: &ReportRow) -> Result<ReportRow, ReportRepositoryError>;
    /// Update a report and return exactly what was stored in the database.
    async fn update(&self, row: &ReportRow) -> Result<ReportRow, ReportRepositoryError>;
    /// Delete a report by job ID. Returns an error if it doesn't exist.
    async fn delete(&self, job_id: uuid::Uuid) -> Result<(), ReportRepositoryError>;

    /// Create a report inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ReportRow,
    ) -> Result<ReportRow, ReportRepositoryError>;
    /// Update a report inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ReportRow,
    ) -> Result<ReportRow, ReportRepositoryError>;
    /// Delete a report inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<(), ReportRepositoryError>;
    /// Fetch a report by job ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<Option<ReportRow>, ReportRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::ReportRepositoryError;
    use crate::infrastructure::db::database::DatabaseError;

    #[test]
    fn given_database_error_when_converted_should_map_to_storage_unavailable() {
        let err = ReportRepositoryError::from(DatabaseError::Query("boom".to_string()));
        assert_eq!(err, ReportRepositoryError::StorageUnavailable);
    }
}
