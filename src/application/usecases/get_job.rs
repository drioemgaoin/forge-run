// Use case: get_job.

use crate::domain::entities::job::Job;
use crate::domain::value_objects::ids::JobId;
use crate::infrastructure::db::dto::JobRow;
use crate::infrastructure::db::stores::job_store::JobStore;

/// Fetches a job by its ID.
pub struct GetJobUseCase<S: JobStore> {
    pub store: S,
}

#[derive(Debug)]
pub enum GetJobError {
    NotFound,
    Storage(String),
}

impl<S: JobStore + Send + Sync> GetJobUseCase<S> {
    /// Get a job by ID.
    pub async fn execute(&self, job_id: JobId) -> Result<Job, GetJobError> {
        // Step 1: Fetch the row from storage.
        let row = self
            .store
            .get(job_id.0)
            .await
            .map_err(|e| GetJobError::Storage(format!("{e:?}")))?;

        // Step 2: Return NotFound when missing.
        let row = row.ok_or(GetJobError::NotFound)?;

        // Step 3: Convert row to domain entity and return.
        Ok(JobRow::into_job(row))
    }
}

#[cfg(test)]
mod tests {
    use super::{GetJobError, GetJobUseCase};
    use crate::domain::entities::job::Job;
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct DummyStore {
        row: Mutex<Option<JobRow>>,
    }

    impl DummyStore {
        fn new(row: Option<JobRow>) -> Self {
            Self {
                row: Mutex::new(row),
            }
        }
    }

    #[async_trait]
    impl JobStore for DummyStore {
        async fn get(&self, _job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError> {
            Ok(self.row.lock().unwrap().clone())
        }

        async fn insert(&self, _row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn update(&self, _row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn delete(&self, _job_id: uuid::Uuid) -> Result<(), JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn list_due_deferred(
            &self,
            _now: time::OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn claim_next_queued(&self) -> Result<Option<JobRow>, JobRepositoryError> {
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
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }
    }

    fn sample_job_row() -> JobRow {
        let job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        JobRow::from_job(&job)
    }

    #[tokio::test]
    async fn given_existing_job_when_execute_should_return_job() {
        let row = sample_job_row();
        let store = DummyStore::new(Some(row.clone()));
        let usecase = GetJobUseCase { store };

        let job = usecase.execute(JobId(row.id)).await.unwrap();

        assert_eq!(job.id.0, row.id);
    }

    #[tokio::test]
    async fn given_missing_job_when_execute_should_return_not_found() {
        let store = DummyStore::new(None);
        let usecase = GetJobUseCase { store };

        let result = usecase.execute(JobId::new()).await;

        assert!(matches!(result, Err(GetJobError::NotFound)));
    }
}
