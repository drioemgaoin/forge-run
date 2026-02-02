// Use case: cancel_job.

use crate::application::context::AppContext;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobState};
use crate::domain::services::job_lifecycle::JobLifecycleError;
use crate::domain::value_objects::ids::JobId;

/// Cancels a job if possible.
pub struct CancelJobUseCase;

#[derive(Debug)]
pub enum CancelJobError {
    NotFound,
    Storage(String),
    Transition(JobLifecycleError),
}

#[derive(Debug)]
pub struct CancelJobResult {
    pub job: Job,
    pub event: Option<Event>,
}

impl CancelJobUseCase {
    /// Cancel a job and emit a cancellation event.
    pub async fn execute(
        ctx: &AppContext,
        job_id: JobId,
    ) -> Result<CancelJobResult, CancelJobError> {
        // Step 1: Fetch the job.
        let row = ctx
            .repos
            .job
            .get(job_id)
            .await
            .map_err(|e| CancelJobError::Storage(format!("{e:?}")))?;

        let row = row.ok_or(CancelJobError::NotFound)?;
        let mut job = row;

        // Step 2: If already canceled, return current state without emitting new event.
        if job.state == JobState::Canceled {
            return Ok(CancelJobResult { job, event: None });
        }

        // Step 3: Transition to Canceled (persists job + event).
        let event = ctx
            .job_lifecycle
            .transition(&mut job, JobState::Canceled)
            .await
            .map_err(CancelJobError::Transition)?;

        // Step 4: Return updated job and event.
        Ok(CancelJobResult {
            job,
            event: Some(event),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CancelJobError, CancelJobUseCase};
    use crate::application::context::test_support::test_context;
    use crate::domain::entities::event::{Event, EventName};
    use crate::domain::entities::job::{Job, JobState};
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

        async fn claim_next_queued(
            &self,
            _worker_id: &str,
            _lease_expires_at: time::OffsetDateTime,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn list_expired_leases(
            &self,
            _now: time::OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn heartbeat(
            &self,
            _job_id: uuid::Uuid,
            _worker_id: &str,
            _heartbeat_at: time::OffsetDateTime,
            _lease_expires_at: time::OffsetDateTime,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn queue_depth(&self) -> Result<u64, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn count_scheduled_at(
            &self,
            _scheduled_at: time::OffsetDateTime,
            _tolerance_ms: u64,
        ) -> Result<u64, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn next_due_time(
            &self,
            _now: time::OffsetDateTime,
        ) -> Result<Option<time::OffsetDateTime>, JobRepositoryError> {
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
            _lease_expires_at: time::OffsetDateTime,
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
            _callback_events: Option<Vec<String>>,
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
            _callback_events: Option<Vec<String>>,
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
                event_name: EventName::JobCanceled,
                prev_state: JobState::Created,
                next_state: JobState::Canceled,
                timestamp: Timestamp::now_utc(),
            })
        }

        async fn finalize_report(
            &self,
            _job_id: JobId,
            _events: Vec<crate::domain::entities::report::EventSnapshot>,
            _outcome: crate::domain::entities::job::JobOutcome,
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
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = state;
        JobRow::from_job(&job)
    }

    #[tokio::test]
    async fn given_active_job_when_execute_should_cancel_and_emit_event() {
        let job_id = JobId::new();
        let row = sample_job_row(JobState::Created, job_id);
        let store = DummyStore {
            row: Mutex::new(Some(row)),
        };
        let mut ctx = test_context();
        ctx.repos.job = std::sync::Arc::new(
            crate::infrastructure::db::repositories::job_repository::JobRepository::new(
                std::sync::Arc::new(store),
            ),
        );
        ctx.job_lifecycle = std::sync::Arc::new(DummyLifecycle);

        let result = CancelJobUseCase::execute(&ctx, job_id).await.unwrap();

        assert_eq!(result.job.state, JobState::Canceled);
        assert!(result.event.is_some());
    }

    #[tokio::test]
    async fn given_canceled_job_when_execute_should_return_without_event() {
        let job_id = JobId::new();
        let row = sample_job_row(JobState::Canceled, job_id);
        let store = DummyStore {
            row: Mutex::new(Some(row)),
        };
        let mut ctx = test_context();
        ctx.repos.job = std::sync::Arc::new(
            crate::infrastructure::db::repositories::job_repository::JobRepository::new(
                std::sync::Arc::new(store),
            ),
        );
        ctx.job_lifecycle = std::sync::Arc::new(DummyLifecycle);

        let result = CancelJobUseCase::execute(&ctx, job_id).await.unwrap();

        assert_eq!(result.job.state, JobState::Canceled);
        assert!(result.event.is_none());
    }

    #[tokio::test]
    async fn given_missing_job_when_execute_should_return_not_found() {
        let store = DummyStore {
            row: Mutex::new(None),
        };
        let mut ctx = test_context();
        ctx.repos.job = std::sync::Arc::new(
            crate::infrastructure::db::repositories::job_repository::JobRepository::new(
                std::sync::Arc::new(store),
            ),
        );
        ctx.job_lifecycle = std::sync::Arc::new(DummyLifecycle);

        let result = CancelJobUseCase::execute(&ctx, JobId::new()).await;

        assert!(matches!(result, Err(CancelJobError::NotFound)));
    }
}
