// Use case: scheduler.

use crate::application::context::AppContext;
use crate::application::usecases::queue_due_jobs::{QueueDueJobsError, QueueDueJobsUseCase};
use crate::domain::value_objects::timestamps::Timestamp;
use time::Duration;

/// Schedules deferred jobs by moving them into the queue when due.
pub struct SchedulerUseCase;

#[derive(Debug)]
pub enum SchedulerError {
    Storage(String),
    Transition(QueueDueJobsError),
}

impl SchedulerUseCase {
    /// Run one scheduling pass and return the number of jobs queued.
    pub async fn run_once(
        ctx: &AppContext,
        now: Timestamp,
        limit: u32,
    ) -> Result<usize, SchedulerError> {
        // Step 1: Queue due deferred jobs.
        let result = QueueDueJobsUseCase::execute(ctx, now, limit)
            .await
            .map_err(SchedulerError::Transition)?;

        // Step 2: Return the number of jobs that were queued.
        Ok(result.jobs.len())
    }

    /// Run the scheduler loop continuously at a fixed interval.
    pub async fn run_loop(
        ctx: &AppContext,
        poll_interval: Duration,
        limit: u32,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), SchedulerError> {
        // Step 1: Loop until shutdown is triggered.
        loop {
            if *shutdown.borrow() {
                break;
            }

            // Step 2: Determine the next due time for deferred jobs.
            let now = Timestamp::now_utc();
            let next_due = ctx
                .repos
                .job
                .next_due_time(now)
                .await
                .map_err(|e| SchedulerError::Storage(format!("{e:?}")))?;

            // Step 3: If something is already due, run a scheduling pass immediately.
            if let Some(next_due) = next_due
                && next_due.as_inner() <= now.as_inner()
            {
                let _ = Self::run_once(ctx, now, limit).await?;
                continue;
            }

            // Step 4: Sleep until the next due job or the regular poll interval.
            let sleep_duration = if let Some(next_due) = next_due {
                let diff = next_due.as_inner() - now.as_inner();
                std::time::Duration::from_millis(diff.whole_milliseconds().max(0) as u64)
            } else {
                std::time::Duration::from_millis(poll_interval.whole_milliseconds().max(0) as u64)
            };
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(sleep_duration) => {}
            }

            // Step 5: Run a scheduling pass after waking.
            let _ = Self::run_once(ctx, Timestamp::now_utc(), limit).await?;
        }

        // Step 6: Exit cleanly.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SchedulerUseCase;
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
    use time::OffsetDateTime;

    struct DummyStore {
        rows: Mutex<Vec<JobRow>>,
    }

    #[async_trait]
    impl JobStore for DummyStore {
        async fn get(&self, _job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
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
            _now: OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Ok(self.rows.lock().unwrap().clone())
        }

        async fn claim_next_queued(
            &self,
            _worker_id: &str,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
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

        async fn count_scheduled_at(
            &self,
            _scheduled_at: OffsetDateTime,
            _tolerance_ms: u64,
        ) -> Result<u64, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn next_due_time(
            &self,
            _now: OffsetDateTime,
        ) -> Result<Option<OffsetDateTime>, JobRepositoryError> {
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
            Ok(Event {
                id: EventId::new(),
                job_id: job.id,
                event_name: EventName::JobQueued,
                prev_state: JobState::Created,
                next_state: JobState::Queued,
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

    fn sample_row() -> JobRow {
        let now = Timestamp::now_utc();
        let execution_at = Timestamp::from(now.as_inner() + time::Duration::seconds(1));
        let job = Job::new_deferred(
            JobId::new(),
            ClientId::new(),
            execution_at,
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        JobRow::from_job(&job)
    }

    #[tokio::test]
    async fn given_due_jobs_when_run_once_should_return_count() {
        let store = DummyStore {
            rows: Mutex::new(vec![sample_row(), sample_row()]),
        };
        let mut ctx = test_context();
        ctx.repos.job = std::sync::Arc::new(
            crate::infrastructure::db::repositories::job_repository::JobRepository::new(
                std::sync::Arc::new(store),
            ),
        );
        ctx.job_lifecycle = std::sync::Arc::new(DummyLifecycle);

        let count = SchedulerUseCase::run_once(&ctx, Timestamp::now_utc(), 10)
            .await
            .unwrap();

        assert_eq!(count, 2);
    }
}
