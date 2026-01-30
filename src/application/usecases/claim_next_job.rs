// Use case: claim_next_job.

use crate::domain::entities::event::Event;
use crate::domain::entities::job::Job;
use crate::domain::value_objects::ids::EventId;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::{EventRow, JobRow};
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::event_store::EventStore;
use crate::infrastructure::db::stores::job_store::JobStore;
use std::sync::Arc;

/// Claims the next queued job and emits the Assigned event.
pub struct ClaimNextJobUseCase<J: JobStore, E: EventStore> {
    pub db: Arc<PostgresDatabase>,
    pub job_store: J,
    pub event_store: E,
}

#[derive(Debug)]
pub enum ClaimNextJobError {
    Storage(String),
}

#[derive(Debug)]
pub struct ClaimNextJobResult {
    pub job: Job,
    pub event: Event,
}

impl<J, E> ClaimNextJobUseCase<J, E>
where
    J: JobStore + Send + Sync + Clone + 'static,
    E: EventStore + Send + Sync + Clone + 'static,
{
    /// Claim the next queued job and emit the Assigned event atomically.
    pub async fn execute(&self) -> Result<Option<ClaimNextJobResult>, ClaimNextJobError> {
        let job_store = self.job_store.clone();
        let event_store = self.event_store.clone();

        // Step 1: Start a transaction and claim the next queued job.
        let result = self
            .db
            .with_tx(|tx| {
                let job_store = job_store.clone();
                let event_store = event_store.clone();
                Box::pin(async move {
                    let Some(job_row) = job_store
                        .claim_next_queued_tx(tx)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))? else {
                        return Ok(None);
                    };

                    // Step 2: Build the Assigned event (Queued -> Assigned).
                    let job = JobRow::into_job(job_row.clone());
                    let event = Event::from_transition(
                        EventId::new(),
                        job.id,
                        crate::domain::entities::job::JobState::Queued,
                        crate::domain::entities::job::JobState::Assigned,
                    )
                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                    let event_row = EventRow::from_event(&event);

                    // Step 3: Persist the event inside the same transaction.
                    event_store
                        .insert_tx(tx, &event_row)
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
    use crate::domain::entities::job::{Job, JobState};
    use crate::domain::entities::event::EventName;
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
    use crate::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::stores::event_store::EventStore;
    use crate::infrastructure::db::stores::job_store::JobStore;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_queued_job_when_execute_should_assign_and_emit_event() {
        let Some(url) = test_db_url() else { return; };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let job_store = JobStorePostgres::new(db.clone());
        let event_store = EventStorePostgres::new(db.clone());
        let usecase = ClaimNextJobUseCase {
            db: db.clone(),
            job_store: job_store.clone(),
            event_store: event_store.clone(),
        };

        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = JobState::Queued;
        job.updated_at = Timestamp::now_utc();
        let row = JobRow::from_job(&job);
        let stored = job_store.insert(&row).await.unwrap();

        let result = usecase.execute().await.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.job.state, JobState::Assigned);
        assert_eq!(result.event.event_name, EventName::JobAssigned);

        let events = event_store.list_by_job_id(stored.id).await.unwrap();
        assert!(!events.is_empty());

        for event in events {
            event_store.delete(event.id).await.unwrap();
        }
        job_store.delete(stored.id).await.unwrap();
    }
}
