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
    async fn claim_next_queued(
        &self,
        worker_id: &str,
        lease_expires_at: OffsetDateTime,
    ) -> Result<Option<JobRow>, JobRepositoryError>;
    /// List jobs with expired leases that should be re-queued.
    async fn list_expired_leases(
        &self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<JobRow>, JobRepositoryError>;
    /// Record a heartbeat for a leased job, extending the lease expiration.
    async fn heartbeat(
        &self,
        job_id: uuid::Uuid,
        worker_id: &str,
        heartbeat_at: OffsetDateTime,
        lease_expires_at: OffsetDateTime,
    ) -> Result<JobRow, JobRepositoryError>;
    /// Return the current number of queued jobs available for workers.
    async fn queue_depth(&self) -> Result<u64, JobRepositoryError>;
    /// Count deferred jobs scheduled around a target time window.
    async fn count_scheduled_at(
        &self,
        scheduled_at: OffsetDateTime,
        tolerance_ms: u64,
    ) -> Result<u64, JobRepositoryError>;
    /// Return the next scheduled deferred execution time, if any.
    async fn next_due_time(
        &self,
        now: OffsetDateTime,
    ) -> Result<Option<OffsetDateTime>, JobRepositoryError>;

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
    /// Atomically claim the next queued job inside an existing transaction.
    async fn claim_next_queued_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        worker_id: &str,
        lease_expires_at: OffsetDateTime,
    ) -> Result<Option<JobRow>, JobRepositoryError>;
}
