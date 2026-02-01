// Use case: claim_next_job.

use crate::application::context::AppContext;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::Job;
use crate::domain::value_objects::ids::EventId;
use crate::infrastructure::db::database::DatabaseError;

/// Claims the next queued job and emits the Assigned event.
pub struct ClaimNextJobUseCase;

#[derive(Debug)]
pub enum ClaimNextJobError {
    Storage(String),
}

#[derive(Debug)]
pub struct ClaimNextJobResult {
    pub job: Job,
    pub event: Event,
}

impl ClaimNextJobUseCase {
    /// Claim the next queued job and emit the Assigned event atomically.
    pub async fn execute(
        ctx: &AppContext,
        worker_id: &str,
        lease_duration: time::Duration,
    ) -> Result<Option<ClaimNextJobResult>, ClaimNextJobError> {
        let job_repo = ctx.repos.job.clone();
        let event_repo = ctx.repos.event.clone();
        // Step 1: Start a transaction and claim the next queued job.
        let result = ctx
            .repos
            .with_tx(|tx| {
                let job_repo = job_repo.clone();
                let event_repo = event_repo.clone();
                let worker_id = worker_id.to_string();
                Box::pin(async move {
                    let Some(job) = job_repo
                        .claim_next_queued_tx(tx, &worker_id, lease_duration)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                    else {
                        return Ok(None);
                    };

                    // Step 2: Build the Assigned event (Queued -> Assigned).
                    let event = Event::from_transition(
                        EventId::new(),
                        job.id,
                        crate::domain::entities::job::JobState::Queued,
                        crate::domain::entities::job::JobState::Assigned,
                    )
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                    // Step 3: Persist the event inside the same transaction.
                    event_repo
                        .insert_tx(tx, &event)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                    // Step 4: Return the claimed job and event.
                    Ok(Some(ClaimNextJobResult { job, event }))
                })
            })
            .await
            .map_err(|e| ClaimNextJobError::Storage(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::ClaimNextJobUseCase;
    use crate::application::context::test_support::test_context;
    use crate::domain::entities::event::EventName;
    use crate::domain::entities::job::{Job, JobState};
    use crate::domain::value_objects::ids::{ClientId, EventId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::repositories::Repositories;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_queued_job_when_execute_should_assign_and_emit_event() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());

        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = JobState::Queued;
        job.updated_at = Timestamp::now_utc();
        let stored = ctx.repos.job.insert(&job).await.unwrap();

        let result = ClaimNextJobUseCase::execute(&ctx, "worker-1", time::Duration::seconds(30))
            .await
            .unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.job.state, JobState::Assigned);
        assert_eq!(result.event.event_name, EventName::JobAssigned);

        let events = ctx.repos.event.list_by_job_id(stored.id).await.unwrap();
        assert!(!events.is_empty());

        for event in events {
            ctx.repos.event.delete(EventId(event.id)).await.unwrap();
        }
        ctx.repos.job.delete(stored.id).await.unwrap();
    }
}
