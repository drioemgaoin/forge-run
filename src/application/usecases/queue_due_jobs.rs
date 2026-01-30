// Use case: queue_due_jobs.

use crate::domain::entities::event::Event;
use crate::domain::entities::job::Job;
use crate::domain::services::job_lifecycle::{JobLifecycleError, JobLifecycleService};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::infrastructure::db::dto::JobRow;
use crate::infrastructure::db::stores::job_store::JobStore;

/// Moves due deferred jobs into the queue.
pub struct QueueDueJobsUseCase<S: JobStore, L: JobLifecycleService> {
    pub job_store: S,
    pub lifecycle: L,
}

#[derive(Debug)]
pub enum QueueDueJobsError {
    Storage(String),
    Transition(JobLifecycleError),
}

#[derive(Debug)]
pub struct QueueDueJobsResult {
    pub jobs: Vec<Job>,
    pub events: Vec<Event>,
}

impl<S: JobStore + Send + Sync, L: JobLifecycleService + Send + Sync> QueueDueJobsUseCase<S, L> {
    /// Find due jobs and move them to `Queued` with events.
    pub async fn execute(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> Result<QueueDueJobsResult, QueueDueJobsError> {
        // Step 1: Load due deferred jobs.
        let rows = self
            .job_store
            .list_due_deferred(now.as_inner(), limit)
            .await
            .map_err(|e| QueueDueJobsError::Storage(format!("{e:?}")))?;

        let mut jobs = Vec::new();
        let mut events = Vec::new();

        // Step 2: Transition each job to queued (persist job + event).
        for row in rows {
            let mut job = JobRow::into_job(row);
            let event = self
                .lifecycle
                .transition(&mut job, crate::domain::entities::job::JobState::Queued)
                .await
                .map_err(QueueDueJobsError::Transition)?;
            jobs.push(job);
            events.push(event);
        }

        // Step 3: Return the list of scheduled jobs and events.
        Ok(QueueDueJobsResult { jobs, events })
    }
}

#[cfg(test)]
mod tests {
    use super::QueueDueJobsUseCase;
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
            Ok(None)
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
    async fn given_due_jobs_when_execute_should_queue_each_one() {
        let store = DummyStore {
            rows: Mutex::new(vec![sample_row()]),
        };
        let usecase = QueueDueJobsUseCase {
            job_store: store,
            lifecycle: DummyLifecycle,
        };

        let result = usecase
            .execute(Timestamp::now_utc(), 10)
            .await
            .unwrap();

        assert_eq!(result.jobs.len(), 1);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.jobs[0].state, JobState::Queued);
    }
}
