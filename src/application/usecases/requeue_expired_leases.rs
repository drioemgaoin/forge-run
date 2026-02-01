// Use case: requeue_expired_leases.

use crate::application::context::AppContext;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::Job;
use crate::domain::services::job_lifecycle::JobLifecycleError;
use crate::domain::value_objects::timestamps::Timestamp;

/// Re-queues jobs whose leases have expired.
pub struct RequeueExpiredLeasesUseCase;

#[derive(Debug)]
pub enum RequeueExpiredLeasesError {
    Storage(String),
    Transition(JobLifecycleError),
}

#[derive(Debug)]
pub struct RequeueExpiredLeasesResult {
    pub jobs: Vec<Job>,
    pub events: Vec<Event>,
}

impl RequeueExpiredLeasesUseCase {
    /// Find expired leases and move jobs back to `Queued` with events.
    pub async fn execute(
        ctx: &AppContext,
        now: Timestamp,
        limit: u32,
    ) -> Result<RequeueExpiredLeasesResult, RequeueExpiredLeasesError> {
        // Step 1: Load jobs whose lease has expired.
        let rows = ctx
            .repos
            .job
            .list_expired_leases(now, limit)
            .await
            .map_err(|e| RequeueExpiredLeasesError::Storage(format!("{e:?}")))?;

        let mut jobs = Vec::new();
        let mut events = Vec::new();

        // Step 2: Clear lease info and transition each job back to queued.
        for mut job in rows {
            job.lease_owner = None;
            job.lease_expires_at = None;
            job.heartbeat_at = None;
            job.outcome = None;
            job.outcome_reason = None;

            let event = ctx
                .job_lifecycle
                .transition(&mut job, crate::domain::entities::job::JobState::Queued)
                .await
                .map_err(RequeueExpiredLeasesError::Transition)?;

            jobs.push(job);
            events.push(event);
        }

        // Step 3: Return the re-queued jobs and events.
        Ok(RequeueExpiredLeasesResult { jobs, events })
    }
}

#[cfg(test)]
mod tests {
    use super::RequeueExpiredLeasesUseCase;
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
            Err(JobRepositoryError::InvalidInput)
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
            Ok(self.rows.lock().unwrap().clone())
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
                prev_state: JobState::Assigned,
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
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = JobState::Assigned;
        JobRow::from_job(&job)
    }

    #[tokio::test]
    async fn given_expired_leases_when_execute_should_requeue_jobs() {
        let store = DummyStore {
            rows: Mutex::new(vec![sample_row()]),
        };
        let mut ctx = test_context();
        ctx.repos.job = std::sync::Arc::new(
            crate::infrastructure::db::repositories::job_repository::JobRepository::new(
                std::sync::Arc::new(store),
            ),
        );
        ctx.job_lifecycle = std::sync::Arc::new(DummyLifecycle);

        let result = RequeueExpiredLeasesUseCase::execute(&ctx, Timestamp::now_utc(), 10)
            .await
            .unwrap();

        assert_eq!(result.jobs.len(), 1);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.jobs[0].state, JobState::Queued);
    }
}
