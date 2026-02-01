// Use case: submit_job.
// Orchestrates validation, persistence, and enqueue.

use crate::application::context::AppContext;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobState, JobType};
use crate::domain::services::job_lifecycle::JobLifecycleError;
use crate::domain::value_objects::ids::{ClientId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::IdempotencyKeyRow;

#[allow(dead_code)]
/// Submits a job and persists the initial event (and idempotency key when provided).
pub struct SubmitJobUseCase;

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

impl SubmitJobUseCase {
    /// Submit a job and persist both the job and its `JobCreated` event.
    pub async fn execute(
        ctx: &AppContext,
        cmd: SubmitJobCommand,
    ) -> Result<SubmitJobResult, JobLifecycleError> {
        let job_repo = ctx.repos.job.clone();
        let event_repo = ctx.repos.event.clone();
        let id_repo = ctx.repos.idempotency.clone();
        let client_id = cmd.client_id;
        let execution_at = cmd.execution_at;
        let callback_url = cmd.callback_url.clone();
        let work_kind = cmd.work_kind.clone();
        let idempotency_key = cmd.idempotency_key.clone();
        let now = Timestamp::now_utc();
        // Step 1: Run the whole submit flow in a single transaction.
        let (job, created_event) = ctx
            .repos
            .with_tx(|tx| {
                let job_repo = job_repo.clone();
                let event_repo = event_repo.clone();
                let id_repo = id_repo.clone();
                let callback_url = callback_url.clone();
                let work_kind = work_kind.clone();
                let idempotency_key = idempotency_key.clone();

                Box::pin(async move {
                    // Step 2: If idempotency key exists, try to reuse existing job + event.
                    if let Some(key) = idempotency_key {
                        if let Some(existing) = id_repo
                            .get_tx(tx, client_id.0, &key)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                            && let Some(existing_job_id) = existing.job_id
                        {
                            let Some(job) = job_repo
                                .get_tx(tx, JobId(existing_job_id))
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                            else {
                                return Err(DatabaseError::Query(
                                    "idempotency_job_missing".to_string(),
                                ));
                            };
                            let Some(event_row) = event_repo
                                .get_by_job_id_and_name_tx(
                                    tx,
                                    JobId(existing_job_id),
                                    "job_created",
                                )
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                            else {
                                return Err(DatabaseError::Query(
                                    "idempotency_event_missing".to_string(),
                                ));
                            };
                            return Ok((job, event_row.into_event()));
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
                        let created_event = Event::new_created(
                            crate::domain::value_objects::ids::EventId::new(),
                            job.id,
                            now,
                        );

                        let id_row = IdempotencyKeyRow {
                            client_id: client_id.0,
                            idempotency_key: key,
                            job_id: Some(job.id.0),
                            created_at: now.as_inner(),
                        };

                        // Step 4: Persist job + event + idempotency entry atomically.
                        job_repo
                            .insert_tx(tx, &job)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        event_repo
                            .insert_tx(tx, &created_event)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        id_repo
                            .insert_tx(tx, &id_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                        // Step 5: Enqueue instant jobs immediately.
                        if job.job_type == JobType::Instant {
                            let mut queued_job = job.clone();
                            queued_job.state = JobState::Queued;
                            queued_job.updated_at = now;
                            let queued_event = Event::from_transition(
                                crate::domain::value_objects::ids::EventId::new(),
                                job.id,
                                JobState::Created,
                                JobState::Queued,
                            )
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                            job_repo
                                .update_tx(tx, &queued_job)
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                            event_repo
                                .insert_tx(tx, &queued_event)
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                            Ok((queued_job, created_event))
                        } else {
                            Ok((job, created_event))
                        }
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
                        let created_event = Event::new_created(
                            crate::domain::value_objects::ids::EventId::new(),
                            job.id,
                            now,
                        );

                        // Step 6: Persist job + event atomically.
                        job_repo
                            .insert_tx(tx, &job)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        event_repo
                            .insert_tx(tx, &created_event)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                        // Step 7: Enqueue instant jobs immediately.
                        if job.job_type == JobType::Instant {
                            let mut queued_job = job.clone();
                            queued_job.state = JobState::Queued;
                            queued_job.updated_at = now;
                            let queued_event = Event::from_transition(
                                crate::domain::value_objects::ids::EventId::new(),
                                job.id,
                                JobState::Created,
                                JobState::Queued,
                            )
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                            job_repo
                                .update_tx(tx, &queued_job)
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                            event_repo
                                .insert_tx(tx, &queued_event)
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                            Ok((queued_job, created_event))
                        } else {
                            Ok((job, created_event))
                        }
                    }
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 7: Return the created job and event.
        Ok(SubmitJobResult { job, created_event })
    }
}

#[cfg(test)]
mod tests {
    use super::{SubmitJobCommand, SubmitJobUseCase};
    use crate::application::context::test_support::test_context;
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::ClientId;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::repositories::Repositories;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_same_idempotency_key_when_execute_should_return_same_job() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();

        let cmd = SubmitJobCommand {
            client_id,
            execution_at: None,
            callback_url: None,
            work_kind: Some("SUCCESS_FAST".to_string()),
            idempotency_key: Some("same-key".to_string()),
        };

        let first = SubmitJobUseCase::execute(&ctx, cmd.clone()).await.unwrap();
        let second = SubmitJobUseCase::execute(&ctx, cmd).await.unwrap();

        assert_eq!(first.job.id, second.job.id);
        assert_eq!(first.created_event.id, second.created_event.id);

        ctx.repos
            .event
            .delete(first.created_event.id)
            .await
            .unwrap();
        ctx.repos.job.delete(first.job.id).await.unwrap();
        ctx.repos
            .idempotency
            .delete(client_id.0, "same-key")
            .await
            .unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }
}
