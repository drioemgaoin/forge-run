use crate::domain::entities::job::Job;
use crate::domain::value_objects::ids::JobId;
use crate::domain::value_objects::timestamps::Timestamp;
use crate::infrastructure::db::dto::JobRow;
use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

pub struct JobRepository {
    store: Arc<dyn JobStore>,
}

impl JobRepository {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<dyn JobStore>) -> Self {
        Self { store }
    }

    /// Create a job and return what was actually stored in the database.
    pub async fn insert(&self, job: &Job) -> Result<Job, JobRepositoryError> {
        // Convert entity to DTO
        let dto = JobRow::from_job(job);

        // Insert and return the stored row from DB
        let stored = self
            .store
            .insert(&dto)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        // Return the job created
        Ok(stored.into_job())
    }

    /// Fetch a job by its ID. Returns `None` if it doesn't exist.
    pub async fn get(&self, job_id: JobId) -> Result<Option<Job>, JobRepositoryError> {
        if let Some(dto) = self
            .store
            .get(job_id.0)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?
        {
            Ok(Some(dto.into_job()))
        } else {
            Ok(None)
        }
    }

    /// Update a job and return what was actually stored in the database.
    pub async fn update(&self, job: &Job) -> Result<Job, JobRepositoryError> {
        // Convert entity to DTO
        let dto = JobRow::from_job(job);

        // Update and return the stored row from DB
        let stored = self
            .store
            .update(&dto)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        // Return the job updated
        Ok(stored.into_job())
    }

    /// Delete a job by its ID. Returns an error if it doesn't exist.
    pub async fn delete(&self, job_id: JobId) -> Result<(), JobRepositoryError> {
        self.store
            .delete(job_id.0)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)
    }

    /// List deferred jobs that should be queued now.
    pub async fn list_due_deferred(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> Result<Vec<Job>, JobRepositoryError> {
        let rows = self
            .store
            .list_due_deferred(now.as_inner(), limit)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        Ok(rows.into_iter().map(JobRow::into_job).collect())
    }

    /// Atomically claim the next queued job for a worker.
    pub async fn claim_next_queued(
        &self,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Option<Job>, JobRepositoryError> {
        // Step 1: Compute the lease expiration timestamp.
        let lease_expires_at = OffsetDateTime::now_utc() + lease_duration;

        // Step 2: Ask the store to claim the next queued job.
        let row = self
            .store
            .claim_next_queued(worker_id, lease_expires_at)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        // Step 3: Map the stored row to the domain entity.
        Ok(row.map(JobRow::into_job))
    }

    /// List jobs whose leases have expired and should be re-queued.
    pub async fn list_expired_leases(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> Result<Vec<Job>, JobRepositoryError> {
        // Step 1: Load expired lease rows from storage.
        let rows = self
            .store
            .list_expired_leases(now.as_inner(), limit)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        // Step 2: Map rows to domain entities.
        Ok(rows.into_iter().map(JobRow::into_job).collect())
    }

    /// Record a heartbeat for a leased job, extending the lease expiration.
    pub async fn heartbeat(
        &self,
        job_id: JobId,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Job, JobRepositoryError> {
        // Step 1: Compute the heartbeat and lease expiration timestamps.
        let now = OffsetDateTime::now_utc();
        let lease_expires_at = now + lease_duration;

        // Step 2: Persist the heartbeat through the store.
        let row = self
            .store
            .heartbeat(job_id.0, worker_id, now, lease_expires_at)
            .await?;

        // Step 3: Return the updated job.
        Ok(row.into_job())
    }

    /// Return the current number of queued jobs available for workers.
    pub async fn queue_depth(&self) -> Result<u64, JobRepositoryError> {
        // Step 1: Ask the store for the current queue depth.
        self.store.queue_depth().await
    }

    /// Fetch a job by its ID inside an existing transaction.
    pub async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: JobId,
    ) -> Result<Option<Job>, JobRepositoryError> {
        let row = self
            .store
            .get_tx(tx, job_id.0)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        Ok(row.map(JobRow::into_job))
    }

    /// Create a job inside an existing transaction and return stored data.
    pub async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job: &Job,
    ) -> Result<Job, JobRepositoryError> {
        let dto = JobRow::from_job(job);
        let stored = self
            .store
            .insert_tx(tx, &dto)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        Ok(stored.into_job())
    }

    /// Update a job inside an existing transaction and return stored data.
    pub async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job: &Job,
    ) -> Result<Job, JobRepositoryError> {
        let dto = JobRow::from_job(job);
        let stored = self
            .store
            .update_tx(tx, &dto)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        Ok(stored.into_job())
    }

    /// Delete a job by its ID inside an existing transaction.
    pub async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: JobId,
    ) -> Result<(), JobRepositoryError> {
        self.store
            .delete_tx(tx, job_id.0)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)
    }

    /// Claim the next queued job inside an existing transaction.
    pub async fn claim_next_queued_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Option<Job>, JobRepositoryError> {
        // Step 1: Compute the lease expiration timestamp.
        let lease_expires_at = OffsetDateTime::now_utc() + lease_duration;

        // Step 2: Claim the job inside the provided transaction.
        let row = self
            .store
            .claim_next_queued_tx(tx, worker_id, lease_expires_at)
            .await
            .map_err(|_| JobRepositoryError::StorageUnavailable)?;

        // Step 3: Map the stored row to the domain entity.
        Ok(row.map(JobRow::into_job))
    }
}

#[cfg(test)]
mod tests {
    use super::JobRepository;
    use crate::domain::entities::job::{Job, JobOutcome, JobState};
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    struct DummyStore {
        pub inserted: Mutex<Option<JobRow>>,
        pub updated: Mutex<Option<JobRow>>,
        pub deleted: Mutex<Option<uuid::Uuid>>,
        pub get_result: Mutex<Option<Option<JobRow>>>,
        pub list_due_result: Mutex<Vec<JobRow>>,
        pub claim_result: Mutex<Option<JobRow>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
                updated: Mutex::new(None),
                deleted: Mutex::new(None),
                get_result: Mutex::new(None),
                list_due_result: Mutex::new(Vec::new()),
                claim_result: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl JobStore for DummyStore {
        async fn get(&self, _job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError> {
            Ok(self.get_result.lock().unwrap().take().unwrap_or(None))
        }

        async fn insert(&self, row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn update(&self, row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            *self.updated.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn delete(&self, job_id: uuid::Uuid) -> Result<(), JobRepositoryError> {
            *self.deleted.lock().unwrap() = Some(job_id);
            Ok(())
        }

        async fn list_due_deferred(
            &self,
            _now: OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Ok(self.list_due_result.lock().unwrap().clone())
        }

        async fn claim_next_queued(
            &self,
            _worker_id: &str,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Ok(self.claim_result.lock().unwrap().take())
        }

        async fn list_expired_leases(
            &self,
            _now: OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn heartbeat(
            &self,
            _job_id: uuid::Uuid,
            _worker_id: &str,
            _heartbeat_at: OffsetDateTime,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn queue_depth(&self) -> Result<u64, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &JobRow,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &JobRow,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<(), JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn claim_next_queued_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _worker_id: &str,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }
    }

    fn sample_job() -> Job {
        Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn given_job_when_insert_should_return_stored_job() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        let job = sample_job();

        let stored = repo.insert(&job).await.unwrap();

        assert_eq!(stored.id, job.id);
        assert_eq!(stored.client_id, job.client_id);
        assert_eq!(stored.job_type, job.job_type);
    }

    #[tokio::test]
    async fn given_job_row_when_get_should_return_job() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        let row = JobRow::from_job(&sample_job());
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let fetched = repo.get(JobId(row.id)).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id.0, row.id);
    }

    #[tokio::test]
    async fn given_missing_job_when_get_should_return_none() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        *store.get_result.lock().unwrap() = Some(None);

        let fetched = repo.get(JobId::new()).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_job_when_update_should_return_updated_job() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        let mut job = sample_job();
        job.state = JobState::Running;
        job.outcome = Some(JobOutcome::Success);

        let updated = repo.update(&job).await.unwrap();

        assert_eq!(updated.state, JobState::Running);
        assert_eq!(updated.outcome, Some(JobOutcome::Success));
    }

    #[tokio::test]
    async fn given_job_id_when_delete_should_call_store() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        let id = JobId::new();

        repo.delete(id).await.unwrap();

        assert_eq!(store.deleted.lock().unwrap().unwrap(), id.0);
    }

    #[tokio::test]
    async fn given_due_rows_when_list_due_deferred_should_map_to_jobs() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        let row = JobRow::from_job(&sample_job());
        store.list_due_result.lock().unwrap().push(row.clone());

        let jobs = repo
            .list_due_deferred(Timestamp::from(OffsetDateTime::now_utc()), 10)
            .await
            .unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id.0, row.id);
    }

    #[tokio::test]
    async fn given_claimed_row_when_claim_next_queued_should_map_to_job() {
        let store = Arc::new(DummyStore::new());
        let repo = JobRepository::new(store.clone());
        let mut row = JobRow::from_job(&sample_job());
        row.state = JobState::Assigned.as_str().to_string();
        *store.claim_result.lock().unwrap() = Some(row.clone());

        let job = repo
            .claim_next_queued("worker-1", time::Duration::seconds(30))
            .await
            .unwrap();

        assert!(job.is_some());
        assert_eq!(job.unwrap().id.0, row.id);
    }
}
