// Use case: submit_job.
// Orchestrates validation, persistence, and enqueue.

use crate::domain::services::job_lifecycle::JobLifecycleError;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::Job;
use crate::domain::value_objects::timestamps::Timestamp;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::{EventRow, IdempotencyKeyRow, JobRow};
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::event_store::EventStore;
use crate::infrastructure::db::stores::idempotency_key_store::IdempotencyKeyStore;
use crate::infrastructure::db::stores::job_store::JobStore;
use crate::domain::value_objects::ids::{ClientId, JobId};
use std::sync::Arc;

#[allow(dead_code)]
/// Submits a job and persists the initial event (and idempotency key when provided).
pub struct SubmitJobUseCase<J: JobStore, E: EventStore, I: IdempotencyKeyStore> {
    pub db: Arc<PostgresDatabase>,
    pub job_store: J,
    pub event_store: E,
    pub idempotency_store: I,
}

#[derive(Debug, Clone)]
pub struct SubmitJobCommand {
    pub client_id: ClientId,
    pub execution_at: Option<Timestamp>,
    pub callback_url: Option<String>,
    pub work_kind: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug)]
pub struct SubmitJobResult {
    pub job: Job,
    pub created_event: Event,
}

impl<J, E, I> SubmitJobUseCase<J, E, I>
where
    J: JobStore + Send + Sync + Clone + 'static,
    E: EventStore + Send + Sync + Clone + 'static,
    I: IdempotencyKeyStore + Send + Sync + Clone + 'static,
{
    /// Submit a job and persist both the job and its `JobCreated` event.
    pub async fn execute(
        &self,
        cmd: SubmitJobCommand,
    ) -> Result<SubmitJobResult, JobLifecycleError> {
        let job_store = self.job_store.clone();
        let event_store = self.event_store.clone();
        let idempotency_store = self.idempotency_store.clone();
        let client_id = cmd.client_id;
        let execution_at = cmd.execution_at;
        let callback_url = cmd.callback_url.clone();
        let work_kind = cmd.work_kind.clone();
        let idempotency_key = cmd.idempotency_key.clone();
        let now = Timestamp::now_utc();

        // Step 1: Run the whole submit flow in a single transaction.
        let (job_row, event_row) = self
            .db
            .with_tx(|tx| {
                let job_store = job_store.clone();
                let event_store = event_store.clone();
                let idempotency_store = idempotency_store.clone();
                let callback_url = callback_url.clone();
                let work_kind = work_kind.clone();
                let idempotency_key = idempotency_key.clone();

                Box::pin(async move {
                    // Step 2: If idempotency key exists, try to reuse existing job + event.
                    if let Some(key) = idempotency_key {
                        if let Some(existing) = idempotency_store
                            .get_tx(tx, client_id.0, &key)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))? {
                            if let Some(existing_job_id) = existing.job_id {
                                let Some(job_row) = job_store
                                    .get_tx(tx, existing_job_id)
                                    .await
                                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))? else {
                                    return Err(DatabaseError::Query(
                                        "idempotency_job_missing".to_string(),
                                    ));
                                };
                                let Some(event_row) = event_store
                                    .get_by_job_id_and_name_tx(tx, existing_job_id, "job_created")
                                    .await
                                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))? else {
                                    return Err(DatabaseError::Query(
                                        "idempotency_event_missing".to_string(),
                                    ));
                                };
                                return Ok((job_row, event_row));
                            }
                        }

                        // Step 3: Build the job + event for a new idempotency key.
                        let job_id = JobId::new();
                        let job = if let Some(execution_at) = execution_at {
                            Job::new_deferred(
                                job_id,
                                client_id,
                                execution_at,
                                callback_url,
                                work_kind,
                            )
                            .map_err(JobLifecycleError::Validation)
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        } else {
                            Job::new_instant(job_id, client_id, callback_url, work_kind)
                                .map_err(JobLifecycleError::Validation)
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        };
                        let created_event =
                            Event::new_created(crate::domain::value_objects::ids::EventId::new(), job.id, now);

                        let job_row = JobRow::from_job(&job);
                        let event_row = EventRow::from_event(&created_event);
                        let id_row = IdempotencyKeyRow {
                            client_id: client_id.0,
                            idempotency_key: key,
                            job_id: Some(job.id.0),
                            created_at: now.as_inner(),
                        };

                        // Step 4: Persist job + event + idempotency entry atomically.
                        job_store
                            .insert_tx(tx, &job_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        event_store
                            .insert_tx(tx, &event_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        idempotency_store
                            .insert_tx(tx, &id_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                        Ok((job_row, event_row))
                    } else {
                        // Step 5: No idempotency key, create job + event only.
                        let job_id = JobId::new();
                        let job = if let Some(execution_at) = execution_at {
                            Job::new_deferred(
                                job_id,
                                client_id,
                                execution_at,
                                callback_url,
                                work_kind,
                            )
                            .map_err(JobLifecycleError::Validation)
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        } else {
                            Job::new_instant(job_id, client_id, callback_url, work_kind)
                                .map_err(JobLifecycleError::Validation)
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        };
                        let created_event =
                            Event::new_created(crate::domain::value_objects::ids::EventId::new(), job.id, now);

                        let job_row = JobRow::from_job(&job);
                        let event_row = EventRow::from_event(&created_event);

                        // Step 6: Persist job + event atomically.
                        job_store
                            .insert_tx(tx, &job_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        event_store
                            .insert_tx(tx, &event_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                        Ok((job_row, event_row))
                    }
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 7: Convert DB rows back to domain objects.
        Ok(SubmitJobResult {
            job: job_row.into_job(),
            created_event: event_row.into_event(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SubmitJobCommand, SubmitJobUseCase};
    use crate::domain::value_objects::ids::ClientId;
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
    use crate::infrastructure::db::postgres::idempotency_key_store_postgres::IdempotencyKeyStorePostgres;
    use crate::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
    use crate::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::stores::client_store::ClientStore;
    use crate::infrastructure::db::stores::event_store::EventStore;
    use crate::infrastructure::db::stores::idempotency_key_store::IdempotencyKeyStore;
    use crate::infrastructure::db::stores::job_store::JobStore;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_same_idempotency_key_when_execute_should_return_same_job() {
        let Some(url) = test_db_url() else { return; };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let job_store = JobStorePostgres::new(db.clone());
        let event_store = EventStorePostgres::new(db.clone());
        let id_store = IdempotencyKeyStorePostgres::new(db.clone());
        let client_store = ClientStorePostgres::new(db.clone());

        let client_id = ClientId::new();
        let client_row = crate::infrastructure::db::dto::ClientRow {
            id: client_id.0,
            created_at: Timestamp::now_utc().as_inner(),
        };
        client_store.insert(&client_row).await.unwrap();

        let usecase = SubmitJobUseCase {
            db: db.clone(),
            job_store: job_store.clone(),
            event_store: event_store.clone(),
            idempotency_store: id_store.clone(),
        };

        let cmd = SubmitJobCommand {
            client_id,
            execution_at: None,
            callback_url: None,
            work_kind: Some("SUCCESS_FAST".to_string()),
            idempotency_key: Some("same-key".to_string()),
        };

        let first = usecase.execute(cmd.clone()).await.unwrap();
        let second = usecase.execute(cmd).await.unwrap();

        assert_eq!(first.job.id, second.job.id);
        assert_eq!(first.created_event.id, second.created_event.id);

        event_store.delete(first.created_event.id.0).await.unwrap();
        job_store.delete(first.job.id.0).await.unwrap();
        id_store
            .delete(client_id.0, "same-key")
            .await
            .unwrap();
        client_store.delete(client_id.0).await.unwrap();
    }
}
