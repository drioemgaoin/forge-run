// Use case: submit_job.
// Orchestrates validation, persistence, and enqueue.

use crate::application::context::AppContext;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobState, JobType, JobValidationError};
use crate::domain::services::job_lifecycle::JobLifecycleError;
use crate::domain::value_objects::ids::{ClientId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::{IdempotencyKeyRow, WebhookDeliveryRow};
use metrics::counter;
use time::Duration;
use tracing::{info, instrument};

#[allow(dead_code)]
/// Submits a job and persists the initial event (and idempotency key when provided).
pub struct SubmitJobUseCase;

#[derive(Debug, Clone)]
pub struct SubmitJobCommand {
    pub client_id: ClientId,
    pub execution_at: Option<Timestamp>,
    pub callback_url: Option<String>,
    pub callback_events: Option<Vec<String>>,
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
    #[instrument(skip(ctx, cmd))]
    pub async fn execute(
        ctx: &AppContext,
        cmd: SubmitJobCommand,
    ) -> Result<SubmitJobResult, JobLifecycleError> {
        let job_repo = ctx.repos.job.clone();
        let event_repo = ctx.repos.event.clone();
        let id_repo = ctx.repos.idempotency.clone();
        let webhook_repo = ctx.repos.webhook.clone();
        let delivery_repo = ctx.repos.webhook_delivery.clone();
        let client_id = cmd.client_id;
        let execution_at = cmd.execution_at;
        let callback_url = cmd.callback_url.clone();
        let callback_events = cmd.callback_events.clone();
        let work_kind = cmd.work_kind.clone();
        let idempotency_key = cmd.idempotency_key.clone();
        let now = Timestamp::now_utc();
        let skew = Duration::seconds(1);
        // Step 0: Apply time-skew tolerance for deferred jobs before the transaction.
        let execution_at = if let Some(execution_at) = execution_at {
            let min_allowed = Timestamp::from(now.as_inner() - skew);
            if execution_at.as_inner() < min_allowed.as_inner() {
                return Err(JobLifecycleError::Validation(
                    crate::domain::entities::job::JobValidationError::ExecutionAtInPast,
                ));
            }
            if execution_at.as_inner() < now.as_inner() {
                Some(now)
            } else {
                Some(execution_at)
            }
        } else {
            None
        };
        // Step 1: Capture admission control settings for deferred jobs.
        let tolerance_ms = ctx.settings.scheduler.tolerance_ms;
        let capacity = ctx.settings.workers.max_count as u64;

        // Step 2: If idempotency key exists, try to reuse existing job + event.
        if let Some(key) = idempotency_key.as_ref()
            && let Some(existing) = id_repo
                .get(client_id.0, key)
                .await
                .map_err(|e| JobLifecycleError::Storage(format!("{e:?}")))?
            && let Some(existing_job_id) = existing.job_id
        {
            let Some(job) = job_repo
                .get(JobId(existing_job_id))
                .await
                .map_err(|e| JobLifecycleError::Storage(format!("{e:?}")))?
            else {
                return Err(JobLifecycleError::Storage(
                    "idempotency_job_missing".to_string(),
                ));
            };
            let Some(event_row) = event_repo
                .get_by_job_id_and_name(JobId(existing_job_id), "job_created")
                .await
                .map_err(|e| JobLifecycleError::Storage(format!("{e:?}")))?
            else {
                return Err(JobLifecycleError::Storage(
                    "idempotency_event_missing".to_string(),
                ));
            };
            counter!("jobs_submitted_total").increment(1);
            info!(
                job_id = %job.id.0,
                client_id = %job.client_id.0,
                "job_submit_idempotent"
            );
            return Ok(SubmitJobResult {
                job,
                created_event: event_row.into_event(),
            });
        }

        // Step 3: Enforce exact-time admission control for deferred jobs.
        if let Some(execution_at) = execution_at {
            let scheduled_count = ctx
                .repos
                .job
                .count_scheduled_at(execution_at, tolerance_ms)
                .await
                .map_err(|e| JobLifecycleError::Storage(format!("{e:?}")))?;
            if scheduled_count >= capacity {
                return Err(JobLifecycleError::Validation(
                    JobValidationError::ScheduleWindowFull,
                ));
            }
        }

        // Step 4: Run the whole submit flow in a single transaction.
        let (job, created_event) = ctx
            .repos
            .with_tx(|tx| {
                let job_repo = job_repo.clone();
                let event_repo = event_repo.clone();
                let id_repo = id_repo.clone();
                let webhook_repo = webhook_repo.clone();
                let delivery_repo = delivery_repo.clone();
                let callback_url = callback_url.clone();
                let callback_events = callback_events.clone();
                let work_kind = work_kind.clone();
                let idempotency_key = idempotency_key.clone();

                Box::pin(async move {
                    // Step 5: If idempotency key exists, try to reuse existing job + event.
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

                        // Step 6: Build the job + event for a new idempotency key.
                        let job_id = JobId::new();
                        let job = if let Some(execution_at) = execution_at {
                            Job::new_deferred(
                                job_id,
                                client_id,
                                execution_at,
                                callback_url,
                                callback_events,
                                work_kind,
                            )
                            .map_err(JobLifecycleError::Validation)
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        } else {
                            Job::new_instant(
                                job_id,
                                client_id,
                                callback_url,
                                callback_events,
                                work_kind,
                            )
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

                        // Step 7: Persist job + event + idempotency entry atomically.
                        job_repo
                            .insert_tx(tx, &job)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        event_repo
                            .insert_tx(tx, &created_event)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        let deliveries =
                            build_webhook_deliveries(&job, &created_event, &webhook_repo).await?;
                        for delivery in deliveries {
                            delivery_repo
                                .insert_tx(tx, &delivery)
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        }
                        id_repo
                            .insert_tx(tx, &id_row)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                        // Step 8: Enqueue instant jobs immediately.
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
                            let deliveries =
                                build_webhook_deliveries(&queued_job, &queued_event, &webhook_repo)
                                    .await?;
                            for delivery in deliveries {
                                delivery_repo
                                    .insert_tx(tx, &delivery)
                                    .await
                                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                            }

                            Ok((queued_job, created_event))
                        } else {
                            Ok((job, created_event))
                        }
                    } else {
                        // Step 9: No idempotency key, create job + event only.
                        let job_id = JobId::new();
                        let job = if let Some(execution_at) = execution_at {
                            Job::new_deferred(
                                job_id,
                                client_id,
                                execution_at,
                                callback_url,
                                callback_events,
                                work_kind,
                            )
                            .map_err(JobLifecycleError::Validation)
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        } else {
                            Job::new_instant(
                                job_id,
                                client_id,
                                callback_url,
                                callback_events,
                                work_kind,
                            )
                            .map_err(JobLifecycleError::Validation)
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                        };
                        let created_event = Event::new_created(
                            crate::domain::value_objects::ids::EventId::new(),
                            job.id,
                            now,
                        );

                        // Step 10: Persist job + event atomically.
                        job_repo
                            .insert_tx(tx, &job)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        event_repo
                            .insert_tx(tx, &created_event)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        let deliveries =
                            build_webhook_deliveries(&job, &created_event, &webhook_repo).await?;
                        for delivery in deliveries {
                            delivery_repo
                                .insert_tx(tx, &delivery)
                                .await
                                .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                        }

                        // Step 11: Enqueue instant jobs immediately.
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
                            let deliveries =
                                build_webhook_deliveries(&queued_job, &queued_event, &webhook_repo)
                                    .await?;
                            for delivery in deliveries {
                                delivery_repo
                                    .insert_tx(tx, &delivery)
                                    .await
                                    .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                            }

                            Ok((queued_job, created_event))
                        } else {
                            Ok((job, created_event))
                        }
                    }
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 7: Emit submission metrics.
        counter!("jobs_submitted_total").increment(1);
        info!(
            job_id = %job.id.0,
            client_id = %job.client_id.0,
            "job_submitted"
        );

        // Step 8: Return the created job and event.
        Ok(SubmitJobResult { job, created_event })
    }
}

async fn build_webhook_deliveries(
    job: &Job,
    event: &Event,
    webhook_repo: &crate::infrastructure::db::repositories::webhook_repository::WebhookRepository,
) -> Result<Vec<WebhookDeliveryRow>, DatabaseError> {
    // Step 1: If job-specific callback is present, use it.
    if let Some(callback_url) = job.callback_url.as_ref() {
        if let Some(events) = job.callback_events.as_ref()
            && !events.iter().any(|evt| evt == event.event_name.as_str())
        {
            return Ok(vec![]);
        }
        let now = event.timestamp.as_inner();
        return Ok(vec![WebhookDeliveryRow {
            id: uuid::Uuid::new_v4(),
            webhook_id: None,
            target_url: callback_url.clone(),
            event_id: event.id.0,
            job_id: event.job_id.0,
            event_name: event.event_name.as_str().to_string(),
            attempt: 0,
            status: "pending".to_string(),
            last_error: None,
            response_status: None,
            next_attempt_at: Some(now),
            created_at: now,
            updated_at: now,
            delivered_at: None,
        }]);
    }

    // Step 2: Load the default webhook for the client.
    let webhook = webhook_repo
        .get_default_for_client(job.client_id.0)
        .await
        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
    let Some(webhook) = webhook else {
        return Ok(vec![]);
    };

    // Step 3: Skip if not subscribed to this event.
    if !webhook
        .events
        .iter()
        .any(|evt| evt == event.event_name.as_str())
    {
        return Ok(vec![]);
    }

    // Step 4: Create the delivery row.
    let now = event.timestamp.as_inner();
    Ok(vec![WebhookDeliveryRow {
        id: uuid::Uuid::new_v4(),
        webhook_id: Some(webhook.id),
        target_url: webhook.url,
        event_id: event.id.0,
        job_id: event.job_id.0,
        event_name: event.event_name.as_str().to_string(),
        attempt: 0,
        status: "pending".to_string(),
        last_error: None,
        response_status: None,
        next_attempt_at: Some(now),
        created_at: now,
        updated_at: now,
        delivered_at: None,
    }])
}

#[cfg(test)]
mod tests {
    use super::{SubmitJobCommand, SubmitJobUseCase};
    use crate::application::context::test_support::test_context;
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::{ClientId, EventId};
    use crate::domain::value_objects::timestamps::Timestamp;
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
            callback_events: None,
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

    #[tokio::test]
    async fn given_execution_at_within_skew_when_execute_should_accept() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();

        let execution_at =
            Timestamp::from(Timestamp::now_utc().as_inner() - time::Duration::milliseconds(500));
        let cmd = SubmitJobCommand {
            client_id,
            execution_at: Some(execution_at),
            callback_url: None,
            callback_events: None,
            work_kind: Some("SUCCESS_FAST".to_string()),
            idempotency_key: None,
        };

        let result = SubmitJobUseCase::execute(&ctx, cmd).await.unwrap();

        assert!(result.job.executed_at.is_some());

        let events = ctx.repos.event.list_by_job_id(result.job.id).await.unwrap();
        for event in events {
            ctx.repos.event.delete(EventId(event.id)).await.unwrap();
        }
        ctx.repos.job.delete(result.job.id).await.unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }

    #[tokio::test]
    async fn given_execution_at_too_far_in_past_when_execute_should_return_error() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();

        let execution_at =
            Timestamp::from(Timestamp::now_utc().as_inner() - time::Duration::seconds(5));
        let cmd = SubmitJobCommand {
            client_id,
            execution_at: Some(execution_at),
            callback_url: None,
            callback_events: None,
            work_kind: Some("SUCCESS_FAST".to_string()),
            idempotency_key: None,
        };

        let result = SubmitJobUseCase::execute(&ctx, cmd).await;

        assert!(matches!(
            result,
            Err(
                crate::domain::services::job_lifecycle::JobLifecycleError::Validation(
                    crate::domain::entities::job::JobValidationError::ExecutionAtInPast
                )
            )
        ));

        ctx.repos.client.delete(client_id).await.unwrap();
    }

    #[tokio::test]
    async fn given_schedule_window_full_when_execute_should_return_error() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();

        let execution_at =
            Timestamp::from(Timestamp::now_utc().as_inner() + time::Duration::seconds(5));
        let existing = crate::domain::entities::job::Job::new_deferred(
            crate::domain::value_objects::ids::JobId::new(),
            client_id,
            execution_at,
            None,
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        ctx.repos.job.insert(&existing).await.unwrap();

        let cmd = SubmitJobCommand {
            client_id,
            execution_at: Some(execution_at),
            callback_url: None,
            callback_events: None,
            work_kind: Some("SUCCESS_FAST".to_string()),
            idempotency_key: None,
        };

        let result = SubmitJobUseCase::execute(&ctx, cmd).await;

        assert!(matches!(
            result,
            Err(
                crate::domain::services::job_lifecycle::JobLifecycleError::Validation(
                    crate::domain::entities::job::JobValidationError::ScheduleWindowFull
                )
            )
        ));

        ctx.repos.job.delete(existing.id).await.unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }
}
