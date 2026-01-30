use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::JobRow;
use async_trait::async_trait;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobRepositoryError {
    NotFound,
    Conflict,
    InvalidInput,
    StorageUnavailable,
}

impl From<DatabaseError> for JobRepositoryError {
    fn from(_: DatabaseError) -> Self {
        JobRepositoryError::StorageUnavailable
    }
}

#[async_trait]
pub trait JobStore: Send + Sync {
    /// Fetch a job by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError>;
    /// Create a job and return exactly what was stored in the database.
    async fn insert(&self, row: &JobRow) -> Result<JobRow, JobRepositoryError>;
    /// Update a job and return exactly what was stored in the database.
    async fn update(&self, row: &JobRow) -> Result<JobRow, JobRepositoryError>;
    /// Delete a job by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, job_id: uuid::Uuid) -> Result<(), JobRepositoryError>;
    /// List deferred jobs that are due for scheduling.
    async fn list_due_deferred(
        &self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<JobRow>, JobRepositoryError>;
    /// Atomically claim the next queued job, if any.
    async fn claim_next_queued(&self) -> Result<Option<JobRow>, JobRepositoryError>;

    /// Create a job inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &JobRow,
    ) -> Result<JobRow, JobRepositoryError>;
    /// Update a job inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &JobRow,
    ) -> Result<JobRow, JobRepositoryError>;
    /// Delete a job inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<(), JobRepositoryError>;
    /// Fetch a job by ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<Option<JobRow>, JobRepositoryError>;
}
