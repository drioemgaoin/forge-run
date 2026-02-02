use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobState, JobValidationError};
use crate::domain::entities::report::{EventSnapshot, Report, ReportError};
use crate::domain::value_objects::ids::{ClientId, EventId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::domain::workflows::state_machine::{JobStateMachine, TransitionError};
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::WebhookDeliveryRow;
use crate::infrastructure::db::repositories::Repositories;
use async_trait::async_trait;
use tracing::info;

/// Domain-level job lifecycle contract.
///
/// This is used by application/use case code to express *what should happen*.
/// It is storage-agnostic and focuses on business rules and events.
/// A concrete implementation (e.g. Postgres) will provide the actual persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobLifecycleError {
    Validation(JobValidationError),
    Transition(TransitionError),
    Report(ReportError),
    Storage(String),
}

/// Job lifecycle orchestration (coordination across Job + Event + Report).
///
/// Use this trait from application/use-case code to describe the business flow.
/// A concrete implementation (like `JobLifecycle`) will handle persistence.
#[async_trait]
pub trait JobLifecycleService: Send + Sync {
    /// Create an instant job and its initial `JobCreated` event.
    ///
    /// Use this in submit flows when the job should run immediately.
    /// If `callback_events` is provided, only those event names trigger the job callback.
    /// Returns both the created job and the created event.
    async fn create_instant(
        &self,
        job_id: JobId,
        client_id: ClientId,
        callback_url: Option<String>,
        callback_events: Option<Vec<String>>,
        work_kind: Option<String>,
    ) -> Result<(Job, Event), JobLifecycleError>;

    /// Create a deferred job and its initial `JobCreated` event.
    ///
    /// Use this in submit flows when the job should run at a future time.
    /// If `callback_events` is provided, only those event names trigger the job callback.
    /// Returns both the created job and the created event.
    async fn create_deferred(
        &self,
        job_id: JobId,
        client_id: ClientId,
        execution_at: Timestamp,
        callback_url: Option<String>,
        callback_events: Option<Vec<String>>,
        work_kind: Option<String>,
    ) -> Result<(Job, Event), JobLifecycleError>;

    /// Transition a job to a new state and emit a transition event.
    ///
    /// Use this when a job moves through the lifecycle (queued, assigned, running, etc.).
    async fn transition(
        &self,
        job: &mut Job,
        next_state: JobState,
    ) -> Result<Event, JobLifecycleError>;

    /// Build and persist a final report for a job.
    ///
    /// Use this after a job has reached a final state (success/failed/canceled).
    async fn finalize_report(
        &self,
        job_id: JobId,
        events: Vec<EventSnapshot>,
        outcome: crate::domain::entities::job::JobOutcome,
        outcome_reason: Option<String>,
    ) -> Result<Report, JobLifecycleError>;
}

/// Domain implementation of the lifecycle using repositories and a transaction runner.
///
/// This is the default implementation used by the application layer.
pub struct JobLifecycle {
    repos: Repositories,
}

impl JobLifecycle {
    /// Build the lifecycle service with repositories that encapsulate persistence.
    pub fn new(repos: Repositories) -> Self {
        // Step 1: Store repositories for later transactional use.
        Self { repos }
    }

    async fn build_webhook_deliveries(
        &self,
        job: &Job,
        event: &Event,
    ) -> Result<Vec<WebhookDeliveryRow>, JobLifecycleError> {
        // Step 1: If a job-specific callback is set, use it exclusively.
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

        // Step 2: Otherwise, load the default webhook for the client.
        let webhook = self
            .repos
            .webhook
            .get_default_for_client(job.client_id.0)
            .await
            .map_err(|e| JobLifecycleError::Storage(format!("{e:?}")))?;

        let Some(webhook) = webhook else {
            return Ok(vec![]);
        };

        // Step 3: Deliver only if the event is subscribed.
        if !webhook
            .events
            .iter()
            .any(|evt| evt == event.event_name.as_str())
        {
            return Ok(vec![]);
        }

        // Step 4: Create the delivery row for the default webhook.
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
}

#[async_trait]
impl JobLifecycleService for JobLifecycle {
    /// Create an instant job and persist both the job and its `JobCreated` event.
    async fn create_instant(
        &self,
        job_id: JobId,
        client_id: ClientId,
        callback_url: Option<String>,
        callback_events: Option<Vec<String>>,
        work_kind: Option<String>,
    ) -> Result<(Job, Event), JobLifecycleError> {
        // Step 1: Build the domain job and the created event.
        let job = Job::new_instant(job_id, client_id, callback_url, callback_events, work_kind)
            .map_err(JobLifecycleError::Validation)?;
        let created_event = Event::new_created(EventId::new(), job.id, Timestamp::now_utc());
        let deliveries = self.build_webhook_deliveries(&job, &created_event).await?;

        // Step 2: Prepare repository handles.
        let job_repo = self.repos.job.clone();
        let event_repo = self.repos.event.clone();
        let delivery_repo = self.repos.webhook_delivery.clone();

        // Step 3: Persist both rows in a single transaction.
        self.repos
            .with_tx(|tx| {
                let job = job.clone();
                let created_event = created_event.clone();
                let deliveries = deliveries.clone();
                Box::pin(async move {
                    job_repo
                        .insert_tx(tx, &job)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    event_repo
                        .insert_tx(tx, &created_event)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    for delivery in deliveries {
                        delivery_repo
                            .insert_tx(tx, &delivery)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    }
                    Ok(())
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 4: Return the created job and event.
        Ok((job, created_event))
    }

    /// Create a deferred job and persist both the job and its `JobCreated` event.
    async fn create_deferred(
        &self,
        job_id: JobId,
        client_id: ClientId,
        execution_at: Timestamp,
        callback_url: Option<String>,
        callback_events: Option<Vec<String>>,
        work_kind: Option<String>,
    ) -> Result<(Job, Event), JobLifecycleError> {
        // Step 1: Build the domain job and the created event.
        let job = Job::new_deferred(
            job_id,
            client_id,
            execution_at,
            callback_url,
            callback_events,
            work_kind,
        )
        .map_err(JobLifecycleError::Validation)?;
        let created_event = Event::new_created(EventId::new(), job.id, Timestamp::now_utc());
        let deliveries = self.build_webhook_deliveries(&job, &created_event).await?;

        // Step 2: Prepare repository handles.
        let job_repo = self.repos.job.clone();
        let event_repo = self.repos.event.clone();
        let delivery_repo = self.repos.webhook_delivery.clone();

        // Step 3: Persist both rows in a single transaction.
        self.repos
            .with_tx(|tx| {
                let job = job.clone();
                let created_event = created_event.clone();
                let deliveries = deliveries.clone();
                Box::pin(async move {
                    job_repo
                        .insert_tx(tx, &job)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    event_repo
                        .insert_tx(tx, &created_event)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    for delivery in deliveries {
                        delivery_repo
                            .insert_tx(tx, &delivery)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    }
                    Ok(())
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 4: Return the created job and event.
        Ok((job, created_event))
    }

    /// Transition a job, persist the job update, and persist the transition event.
    async fn transition(
        &self,
        job: &mut Job,
        next_state: JobState,
    ) -> Result<Event, JobLifecycleError> {
        // Step 1: Validate the transition using the state machine.
        let prev_state = job.state;
        JobStateMachine::transition(prev_state, next_state)
            .map_err(JobLifecycleError::Transition)?;

        // Step 2: Apply the state change to the job (domain rules).
        match next_state {
            JobState::Succeeded => {
                let _ = job.mark_succeeded();
            }
            JobState::Failed => {
                let reason = job.outcome_reason.clone().unwrap_or_default();
                let _ = job.mark_failed(reason);
            }
            JobState::Canceled => {
                job.mark_canceled(job.outcome_reason.clone());
            }
            _ => {
                job.state = next_state;
                job.updated_at = Timestamp::now_utc();
            }
        }

        // Step 3: Create the transition event.
        let event = Event::from_transition(EventId::new(), job.id, prev_state, next_state)
            .map_err(JobLifecycleError::Transition)?;
        let deliveries = self.build_webhook_deliveries(job, &event).await?;

        // Step 4: Prepare repository handles.
        let job_repo = self.repos.job.clone();
        let event_repo = self.repos.event.clone();
        let delivery_repo = self.repos.webhook_delivery.clone();

        // Step 5: Persist both rows in a single transaction.
        self.repos
            .with_tx(|tx| {
                let job = job.clone();
                let event = event.clone();
                let deliveries = deliveries.clone();
                Box::pin(async move {
                    job_repo
                        .update_tx(tx, &job)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    event_repo
                        .insert_tx(tx, &event)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    for delivery in deliveries {
                        delivery_repo
                            .insert_tx(tx, &delivery)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    }
                    Ok(())
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 6: Emit a transition log and return the created event.
        info!(
            job_id = %job.id.0,
            event = %event.event_name.as_str(),
            state = %job.state.as_str(),
            "job_transitioned"
        );
        Ok(event)
    }

    /// Build and persist a job report in the database.
    async fn finalize_report(
        &self,
        job_id: JobId,
        events: Vec<EventSnapshot>,
        outcome: crate::domain::entities::job::JobOutcome,
        outcome_reason: Option<String>,
    ) -> Result<Report, JobLifecycleError> {
        // Step 1: Build the report from events (domain validation).
        let report = Report::from_events(job_id, outcome, outcome_reason, events)
            .map_err(JobLifecycleError::Report)?;

        // Step 2: Prepare repository handle.
        let report_repo = self.repos.report.clone();

        // Step 3: Persist the report in a single transaction.
        self.repos
            .with_tx(|tx| {
                let report = report.clone();
                Box::pin(async move {
                    report_repo
                        .insert_tx(tx, &report)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    Ok(())
                })
            })
            .await
            .map_err(|e| JobLifecycleError::Storage(e.to_string()))?;

        // Step 4: Return the report.
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::{JobLifecycle, JobLifecycleError, JobLifecycleService};
    use crate::domain::entities::event::EventName;
    use crate::domain::entities::job::{JobOutcome, JobState, JobValidationError};
    use crate::domain::entities::report::EventSnapshot;
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::repositories::Repositories;
    use std::sync::Arc;
    use time::Duration;

    async fn setup_lifecycle() -> Option<(JobLifecycle, Repositories)> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let db = Arc::new(PostgresDatabase::connect(&url).await.ok()?);
        let repos = Repositories::postgres(db.clone());
        let lifecycle = JobLifecycle::new(repos.clone());
        Some((lifecycle, repos))
    }

    #[tokio::test]
    async fn given_valid_instant_job_when_created_should_return_job() {
        let Some((lifecycle, repos)) = setup_lifecycle().await else {
            return;
        };
        let (job, event) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(job.state, JobState::Created);
        assert_eq!(event.event_name, EventName::JobCreated);
        assert_eq!(event.job_id, job.id);

        repos.event.delete(event.id).await.unwrap();
        repos.job.delete(job.id).await.unwrap();
    }

    #[tokio::test]
    async fn given_missing_work_kind_when_created_instant_should_return_error() {
        let Some((lifecycle, _repos)) = setup_lifecycle().await else {
            return;
        };
        let result = lifecycle
            .create_instant(JobId::new(), ClientId::new(), None, None, None)
            .await;

        assert!(matches!(
            result,
            Err(JobLifecycleError::Validation(
                JobValidationError::MissingWorkKind
            ))
        ));
    }

    #[tokio::test]
    async fn given_past_execution_at_when_created_deferred_should_return_error() {
        let Some((lifecycle, _repos)) = setup_lifecycle().await else {
            return;
        };
        let now = Timestamp::now_utc();
        let execution_at = Timestamp::from(now.as_inner() - Duration::seconds(1));

        let result = lifecycle
            .create_deferred(
                JobId::new(),
                ClientId::new(),
                execution_at,
                None,
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .await;

        assert!(matches!(
            result,
            Err(JobLifecycleError::Validation(
                JobValidationError::ExecutionAtInPast
            ))
        ));
    }

    #[tokio::test]
    async fn given_valid_transition_when_called_should_update_state_and_emit_event() {
        let Some((lifecycle, repos)) = setup_lifecycle().await else {
            return;
        };
        let (mut job, created_event) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .await
            .unwrap();

        let event = lifecycle
            .transition(&mut job, JobState::Queued)
            .await
            .unwrap();

        assert_eq!(job.state, JobState::Queued);
        assert_eq!(event.event_name, EventName::JobQueued);

        repos.event.delete(event.id).await.unwrap();
        repos.event.delete(created_event.id).await.unwrap();
        repos.job.delete(job.id).await.unwrap();
    }

    #[tokio::test]
    async fn given_invalid_transition_when_called_should_return_error() {
        let Some((lifecycle, repos)) = setup_lifecycle().await else {
            return;
        };
        let (mut job, created_event) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .await
            .unwrap();

        let result = lifecycle.transition(&mut job, JobState::Running).await;

        assert_eq!(
            result,
            Err(JobLifecycleError::Transition(
                crate::domain::workflows::state_machine::TransitionError::Forbidden
            ))
        );

        repos.event.delete(created_event.id).await.unwrap();
        repos.job.delete(job.id).await.unwrap();
    }

    #[tokio::test]
    async fn given_running_to_succeeded_when_transitioned_should_set_outcome() {
        let Some((lifecycle, repos)) = setup_lifecycle().await else {
            return;
        };
        let (mut job, created_event) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .await
            .unwrap();
        job.state = JobState::Running;

        let event = lifecycle
            .transition(&mut job, JobState::Succeeded)
            .await
            .unwrap();

        assert_eq!(job.state, JobState::Succeeded);
        assert_eq!(job.outcome, Some(JobOutcome::Success));
        assert_eq!(event.event_name, EventName::JobSucceeded);

        repos.event.delete(event.id).await.unwrap();
        repos.event.delete(created_event.id).await.unwrap();
        repos.job.delete(job.id).await.unwrap();
    }

    #[tokio::test]
    async fn given_valid_events_when_finalize_report_called_should_return_report() {
        let Some((lifecycle, repos)) = setup_lifecycle().await else {
            return;
        };
        let (job, created_event) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .await
            .unwrap();
        let job_id = job.id;
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + Duration::seconds(1));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t1,
            },
        ];

        let report = lifecycle
            .finalize_report(job_id, events, JobOutcome::Success, None)
            .await
            .unwrap();

        assert_eq!(report.job_id, job_id);
        assert_eq!(report.outcome, JobOutcome::Success);

        repos.report.delete(job_id).await.unwrap();
        repos.event.delete(created_event.id).await.unwrap();
        repos.job.delete(job_id).await.unwrap();
    }

    #[tokio::test]
    async fn given_missing_start_event_when_finalize_report_called_should_return_error() {
        let Some((lifecycle, _repos)) = setup_lifecycle().await else {
            return;
        };
        let job_id = JobId::new();
        let t1 = Timestamp::now_utc();

        let events = vec![EventSnapshot {
            event_name: EventName::JobSucceeded,
            prev_state: JobState::Running,
            next_state: JobState::Succeeded,
            timestamp: t1,
        }];

        let result = lifecycle
            .finalize_report(job_id, events, JobOutcome::Success, None)
            .await;

        assert_eq!(
            result,
            Err(JobLifecycleError::Report(
                crate::domain::entities::report::ReportError::MissingStartedEvent
            ))
        );
    }
}
