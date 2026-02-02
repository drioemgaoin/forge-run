// Use case: run_worker_once.

use crate::application::context::AppContext;
use crate::application::usecases::claim_next_job::ClaimNextJobUseCase;
use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobOutcome, JobState};
use crate::domain::entities::report::{EventSnapshot, Report};
use crate::domain::services::job_lifecycle::JobLifecycleError;
use crate::domain::value_objects::timestamps::Timestamp;
use crate::domain::workflows::retry_policy::RetryPolicy;
use crate::domain::workflows::work_catalog::{WorkCatalog, WorkPlan};
use metrics::counter;
use time::Duration;
use tracing::{info, instrument};

/// Configuration for worker execution and retries.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub lease_timeout: Duration,
    pub retry_policy: RetryPolicy,
    pub max_runtime_ms: u64,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            lease_timeout: Duration::seconds(30),
            retry_policy: RetryPolicy::default(),
            max_runtime_ms: WorkCatalog::DEFAULT_MAX_RUNTIME_MS,
        }
    }
}

/// Executes a single job from the queue (if available).
pub struct RunWorkerOnceUseCase;

#[derive(Debug)]
pub enum RunWorkerOnceError {
    Storage(String),
    Transition(JobLifecycleError),
    UnknownWorkKind,
}

#[derive(Debug)]
pub struct RunWorkerOnceResult {
    pub job: Job,
    pub events: Vec<Event>,
    pub report: Option<Report>,
}

impl RunWorkerOnceUseCase {
    /// Claim the next job, execute it, and persist all state transitions.
    #[instrument(skip(ctx, config))]
    pub async fn execute(
        ctx: &AppContext,
        worker_id: &str,
        config: &WorkerConfig,
    ) -> Result<Option<RunWorkerOnceResult>, RunWorkerOnceError> {
        // Step 1: Claim the next queued job with a lease.
        let claimed = ClaimNextJobUseCase::execute(ctx, worker_id, config.lease_timeout)
            .await
            .map_err(|e| RunWorkerOnceError::Storage(format!("{e:?}")))?;

        let Some(result) = claimed else {
            return Ok(None);
        };

        let mut job = result.job;
        let mut events = vec![result.event];

        // Step 2: Transition to running before executing.
        job.heartbeat_at = Some(Timestamp::now_utc());
        job.lease_expires_at = Some(Timestamp::from(
            Timestamp::now_utc().as_inner() + config.lease_timeout,
        ));

        let started = ctx
            .job_lifecycle
            .transition(&mut job, JobState::Running)
            .await
            .map_err(RunWorkerOnceError::Transition)?;
        events.push(started);

        // Step 3: Resolve the work plan for this job.
        let work_kind = job
            .working_kind
            .clone()
            .ok_or(RunWorkerOnceError::UnknownWorkKind)?;
        let plan = WorkCatalog::evaluate(&work_kind, job.attempt, config.max_runtime_ms)
            .ok_or(RunWorkerOnceError::UnknownWorkKind)?;

        // Step 4: Apply the execution result and handle retry if needed.
        let result = Self::apply_execution(ctx, &mut job, plan, &config.retry_policy).await?;
        let mut all_events = events;
        all_events.extend(result.events);

        // Step 5: Emit execution metrics and logs.
        let outcome = result.job.outcome.map(|o| o.as_str().to_string());
        counter!("jobs_processed_total").increment(1);
        if result.job.state == JobState::Succeeded {
            counter!("jobs_succeeded_total").increment(1);
        } else if result.job.state == JobState::Failed {
            counter!("jobs_failed_total").increment(1);
        } else if result.job.state == JobState::Canceled {
            counter!("jobs_canceled_total").increment(1);
        }
        info!(
            job_id = %result.job.id.0,
            state = %result.job.state.as_str(),
            outcome = outcome.as_deref().unwrap_or(""),
            "job_execution_complete"
        );

        // Step 6: Return the final job, events, and optional report.
        Ok(Some(RunWorkerOnceResult {
            job: result.job,
            events: all_events,
            report: result.report,
        }))
    }

    /// Apply the work plan outcome to the job and produce terminal effects.
    async fn apply_execution(
        ctx: &AppContext,
        job: &mut Job,
        plan: WorkPlan,
        retry_policy: &RetryPolicy,
    ) -> Result<RunWorkerOnceResult, RunWorkerOnceError> {
        let mut events = Vec::new();

        // Step 1: Clear the lease info before final transitions.
        job.lease_owner = None;
        job.lease_expires_at = None;
        job.heartbeat_at = None;

        // Step 2: Apply outcome-specific transitions.
        match plan.outcome {
            JobOutcome::Success => {
                let event = ctx
                    .job_lifecycle
                    .transition(job, JobState::Succeeded)
                    .await
                    .map_err(RunWorkerOnceError::Transition)?;
                events.push(event);
            }
            JobOutcome::Canceled => {
                let event = ctx
                    .job_lifecycle
                    .transition(job, JobState::Canceled)
                    .await
                    .map_err(RunWorkerOnceError::Transition)?;
                events.push(event);
            }
            JobOutcome::Failed => {
                job.outcome_reason = plan.outcome_reason.clone();
                let failed_event = ctx
                    .job_lifecycle
                    .transition(job, JobState::Failed)
                    .await
                    .map_err(RunWorkerOnceError::Transition)?;
                events.push(failed_event);

                if plan.retryable && retry_policy.can_retry(job.attempt) {
                    // Step 3: Prepare retry with backoff.
                    let next_attempt = job.attempt.saturating_add(1);
                    let seed = job.id.0.as_u128() as u64;
                    let delay = retry_policy.next_delay(next_attempt, seed);
                    let next_run = Timestamp::from(Timestamp::now_utc().as_inner() + delay);

                    job.attempt = next_attempt;
                    job.outcome = None;
                    job.outcome_reason = None;
                    job.executed_at = Some(next_run);

                    let queued_event = ctx
                        .job_lifecycle
                        .transition(job, JobState::Queued)
                        .await
                        .map_err(RunWorkerOnceError::Transition)?;
                    events.push(queued_event);

                    return Ok(RunWorkerOnceResult {
                        job: job.clone(),
                        events,
                        report: None,
                    });
                }
            }
        }

        // Step 3: Build the report for terminal outcomes.
        let snapshots = load_event_snapshots(ctx, job.id)
            .await
            .map_err(|e| RunWorkerOnceError::Storage(format!("{e:?}")))?;
        let report = ctx
            .job_lifecycle
            .finalize_report(job.id, snapshots, plan.outcome, plan.outcome_reason)
            .await
            .map_err(RunWorkerOnceError::Transition)?;

        Ok(RunWorkerOnceResult {
            job: job.clone(),
            events,
            report: Some(report),
        })
    }
}

/// Load job events and map them into report snapshots.
async fn load_event_snapshots(
    ctx: &AppContext,
    job_id: crate::domain::value_objects::ids::JobId,
) -> Result<Vec<EventSnapshot>, JobLifecycleError> {
    // Step 1: Load all events for the job.
    let mut rows = ctx
        .repos
        .event
        .list_by_job_id(job_id)
        .await
        .map_err(|e| JobLifecycleError::Storage(format!("{e:?}")))?;

    // Step 2: Sort by timestamp for deterministic reports.
    rows.sort_by_key(|row| row.occurred_at);

    // Step 3: Map to report snapshots.
    Ok(rows
        .into_iter()
        .map(|row| row.into_event())
        .map(|event| EventSnapshot {
            event_name: event.event_name,
            prev_state: event.prev_state,
            next_state: event.next_state,
            timestamp: event.timestamp,
        })
        .collect())
}
