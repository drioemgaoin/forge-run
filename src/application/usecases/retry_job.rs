// Use case: retry_job.

use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobState};
use crate::domain::services::job_lifecycle::{JobLifecycleError, JobLifecycleService};
use crate::domain::value_objects::ids::JobId;
use crate::infrastructure::db::dto::JobRow;
use crate::infrastructure::db::stores::job_store::JobStore;

/// Retries a failed job (moves it back to queued).
pub struct RetryJobUseCase<S: JobStore, L: JobLifecycleService> {
    pub job_store: S,
    pub lifecycle: L,
}

#[derive(Debug)]
pub enum RetryJobError {
    NotFound,
    InvalidState,
    Storage(String),
    Transition(JobLifecycleError),
}

#[derive(Debug)]
pub struct RetryJobResult {
    pub job: Job,
    pub event: Option<Event>,
}

impl<S: JobStore + Send + Sync, L: JobLifecycleService + Send + Sync> RetryJobUseCase<S, L> {
    /// Retry a job and emit a queued event if allowed.
    pub async fn execute(&self, job_id: JobId) -> Result<RetryJobResult, RetryJobError> {
        // Step 1: Fetch the job.
        let row = self
            .job_store
            .get(job_id.0)
            .await
            .map_err(|e| RetryJobError::Storage(format!("{e:?}")))?;

        let row = row.ok_or(RetryJobError::NotFound)?;
        let mut job = JobRow::into_job(row);

        // Step 2: If not failed, return current state without emitting new event.
        if job.state != JobState::Failed {
            return Err(RetryJobError::InvalidState);
        }

        // Step 3: Update attempt and clear outcome before retry.
        job.attempt = job.attempt.saturating_add(1);
        job.outcome = None;
        job.outcome_reason = None;

        // Step 4: Transition to Queued (persists job + event).
        let event = self
            .lifecycle
            .transition(&mut job, JobState::Queued)
            .await
            .map_err(RetryJobError::Transition)?;

        // Step 5: Return updated job and event.
        Ok(RetryJobResult {
            job,
            event: Some(event),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{RetryJobError, RetryJobUseCase};
    use crate::domain::entities::event::{Event, EventName};
    use crate::domain::entities::job::{Job, JobOutcome, JobState};
    use crate::domain::services::job_lifecycle::{JobLifecycleError, JobLifecycleService};
    use crate::domain::value_objects::ids::{ClientId, EventId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct DummyStore {
        row: Mutex<Option<JobRow>>,
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

    struct DummyLifecycle;

    #[async_trait]
    impl JobLifecycleService for DummyLifecycle {
        async fn create_instant(
            &self,
            _job_id: JobId,
            _client_id: ClientId,
            _callback_url: Option<String>,
            _work_kind: Option<String>,
        ) -> Result<(Job, Event), JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }

        async fn create_deferred(
            &self,
            _job_id: JobId,
            _client_id: ClientId,
            _execution_at: Timestamp,
            _callback_url: Option<String>,
            _work_kind: Option<String>,
        ) -> Result<(Job, Event), JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }

        async fn transition(
            &self,
            job: &mut Job,
            next_state: JobState,
        ) -> Result<Event, JobLifecycleError> {
            job.state = next_state;
            job.updated_at = Timestamp::now_utc();
            Ok(Event {
                id: EventId::new(),
                job_id: job.id,
                event_name: EventName::JobQueued,
                prev_state: JobState::Failed,
                next_state: JobState::Queued,
                timestamp: Timestamp::now_utc(),
            })
        }

        async fn finalize_report(
            &self,
            _job_id: JobId,
            _events: Vec<crate::domain::entities::report::EventSnapshot>,
            _outcome: JobOutcome,
            _outcome_reason: Option<String>,
        ) -> Result<crate::domain::entities::report::Report, JobLifecycleError> {
            Err(JobLifecycleError::Storage("unused".to_string()))
        }
    }

    fn sample_job_row(state: JobState, job_id: JobId) -> JobRow {
        let mut job = Job::new_instant(
            job_id,
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = state;
        job.outcome = Some(JobOutcome::Failed);
        JobRow::from_job(&job)
    }

    #[tokio::test]
    async fn given_failed_job_when_execute_should_retry_and_emit_event() {
        let job_id = JobId::new();
        let row = sample_job_row(JobState::Failed, job_id);
        let store = DummyStore {
            row: Mutex::new(Some(row)),
        };
        let usecase = RetryJobUseCase {
            job_store: store,
            lifecycle: DummyLifecycle,
        };

        let result = usecase.execute(job_id).await.unwrap();

        assert_eq!(result.job.state, JobState::Queued);
        assert!(result.event.is_some());
    }

    #[tokio::test]
    async fn given_non_failed_job_when_execute_should_return_invalid_state() {
        let job_id = JobId::new();
        let row = sample_job_row(JobState::Created, job_id);
        let store = DummyStore {
            row: Mutex::new(Some(row)),
        };
        let usecase = RetryJobUseCase {
            job_store: store,
            lifecycle: DummyLifecycle,
        };

        let result = usecase.execute(job_id).await;

        assert!(matches!(result, Err(RetryJobError::InvalidState)));
    }

    #[tokio::test]
    async fn given_missing_job_when_execute_should_return_not_found() {
        let store = DummyStore {
            row: Mutex::new(None),
        };
        let usecase = RetryJobUseCase {
            job_store: store,
            lifecycle: DummyLifecycle,
        };

        let result = usecase.execute(JobId::new()).await;

        assert!(matches!(result, Err(RetryJobError::NotFound)));
    }
}
