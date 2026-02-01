use crate::domain::value_objects::ids::{ClientId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    Instant,
    Deferred,
}

impl JobType {
    pub fn as_str(self) -> &'static str {
        match self {
            JobType::Instant => "instant",
            JobType::Deferred => "deferred",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Created,
    Queued,
    Assigned,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Created => "created",
            JobState::Queued => "queued",
            JobState::Assigned => "assigned",
            JobState::Running => "running",
            JobState::Succeeded => "succeeded",
            JobState::Failed => "failed",
            JobState::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobOutcome {
    Success,
    Failed,
    Canceled,
}

impl JobOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            JobOutcome::Success => "success",
            JobOutcome::Failed => "failed",
            JobOutcome::Canceled => "canceled",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobValidationError {
    MissingExecutionAt,
    ExecutionAtInPast,
    ExecutionAtNotAllowed,
    MissingWorkKind,
    InvalidWorkKind,
    InvalidCallbackUrl,
    ScheduleWindowFull,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub client_id: ClientId,
    pub job_type: JobType,
    pub state: JobState,
    pub attempt: u8,
    pub outcome: Option<JobOutcome>,
    pub outcome_reason: Option<String>,
    pub executed_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<Timestamp>,
    pub heartbeat_at: Option<Timestamp>,
    pub callback_url: Option<String>,
    pub working_kind: Option<String>,
}

impl Job {
    fn new(
        id: JobId,
        client_id: ClientId,
        job_type: JobType,
        executed_at: Option<Timestamp>,
        callback_url: Option<String>,
        working_kind: Option<String>,
        now: Timestamp,
    ) -> Result<Self, JobValidationError> {
        let working_kind = match working_kind {
            Some(kind) => {
                if !Self::is_valid_work_kind(&kind) {
                    return Err(JobValidationError::InvalidWorkKind);
                }
                Some(kind)
            }
            None => return Err(JobValidationError::MissingWorkKind),
        };

        if let Some(callback) = callback_url.as_deref()
            && !Self::is_valid_callback_url(callback)
        {
            return Err(JobValidationError::InvalidCallbackUrl);
        }

        Ok(Self {
            id,
            client_id,
            job_type,
            state: JobState::Created,
            attempt: 0,
            outcome: None,
            outcome_reason: None,
            executed_at,
            created_at: now,
            updated_at: now,
            lease_owner: None,
            lease_expires_at: None,
            heartbeat_at: None,
            callback_url,
            working_kind,
        })
    }

    pub fn new_instant(
        id: JobId,
        client_id: ClientId,
        callback_url: Option<String>,
        working_kind: Option<String>,
    ) -> Result<Self, JobValidationError> {
        Self::new(
            id,
            client_id,
            JobType::Instant,
            None,
            callback_url,
            working_kind,
            Timestamp::now_utc(),
        )
    }

    pub fn new_deferred(
        id: JobId,
        client_id: ClientId,
        execution_at: Timestamp,
        callback_url: Option<String>,
        working_kind: Option<String>,
    ) -> Result<Self, JobValidationError> {
        let now = Timestamp::now_utc();
        if execution_at.as_inner() < now.as_inner() {
            return Err(JobValidationError::ExecutionAtInPast);
        }

        Self::new(
            id,
            client_id,
            JobType::Deferred,
            Some(execution_at),
            callback_url,
            working_kind,
            now,
        )
    }

    pub fn mark_succeeded(&mut self) -> Result<(), JobValidationError> {
        self.state = JobState::Succeeded;
        self.outcome = Some(JobOutcome::Success);
        self.outcome_reason = None;
        self.updated_at = Timestamp::now_utc();

        Ok(())
    }

    pub fn mark_failed(&mut self, reason: String) -> Result<(), JobValidationError> {
        self.state = JobState::Failed;
        self.outcome = Some(JobOutcome::Failed);
        self.outcome_reason = Some(reason);
        self.updated_at = Timestamp::now_utc();

        Ok(())
    }

    pub fn mark_canceled(&mut self, reason: Option<String>) {
        self.state = JobState::Canceled;
        self.outcome = Some(JobOutcome::Canceled);
        self.outcome_reason = reason;
        self.updated_at = Timestamp::now_utc();
    }

    fn is_valid_callback_url(callback_url: &str) -> bool {
        let trimmed = callback_url.trim();
        let without_scheme = if let Some(rest) = trimmed.strip_prefix("http://") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("https://") {
            rest
        } else {
            return false;
        };

        let host = without_scheme.split('/').next().unwrap_or_default();
        !host.is_empty()
    }

    fn is_valid_work_kind(work_kind: &str) -> bool {
        matches!(
            work_kind,
            "SUCCESS_FAST"
                | "SUCCESS_NORMAL"
                | "SUCCESS_SLOW"
                | "FAIL_IMMEDIATE"
                | "FAIL_AFTER_PROGRESS"
                | "FAIL_AFTER_RETRYABLE"
                | "RUNS_LONG"
                | "RUNS_OVER_TIMEOUT"
                | "CPU_BURST"
                | "MEMORY_SPIKE"
                | "IO_HEAVY"
                | "MANY_SMALL_OUTPUTS"
                | "LARGE_OUTPUT"
                | "CANCEL_BEFORE_START"
                | "CANCEL_DURING_RUN"
                | "RETRY_ON_FAIL"
                | "RETRY_LIMIT_REACHED"
                | "DUPLICATE_SUBMIT_SAME_KEY"
                | "DUPLICATE_SUBMIT_DIFFERENT_KEY"
                | "WEBHOOK_SUCCESS"
                | "WEBHOOK_TIMEOUT"
                | "WEBHOOK_5XX"
                | "WEBHOOK_RETRIES_EXHAUSTED"
                | "WEBHOOK_SLOW_RECEIVER"
                | "SCHEDULED_ON_TIME"
                | "SCHEDULED_LATE_RECOVERY"
                | "SCHEDULED_FAR_FUTURE"
                | "PAYLOAD_SMALL"
                | "PAYLOAD_MEDIUM"
                | "PAYLOAD_LARGE"
                | "PAYLOAD_INVALID"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_new_job_when_created_should_have_initial_state_and_attempt_zero() {
        let job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();

        assert_eq!(job.state, JobState::Created);
        assert_eq!(job.attempt, 0);
        assert_eq!(job.created_at, job.updated_at);
    }

    #[test]
    fn given_deferred_job_when_created_should_have_execution_at_set() {
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

        assert_eq!(job.executed_at, Some(execution_at));
    }

    #[test]
    fn given_instant_job_when_created_should_have_no_execution_at() {
        let job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();

        assert_eq!(job.executed_at, None);
    }

    #[test]
    fn given_deferred_job_when_created_should_have_job_type_deferred() {
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

        assert_eq!(job.job_type, JobType::Deferred);
    }

    #[test]
    fn given_job_with_no_callback_when_created_should_have_none_callback_url() {
        let job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();

        assert_eq!(job.callback_url, None);
    }

    #[test]
    fn given_failed_job_when_outcome_set_should_be_failed() {
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = JobState::Running;
        job.mark_failed("error".to_string()).unwrap();

        assert_eq!(job.state, JobState::Failed);
        assert_eq!(job.outcome, Some(JobOutcome::Failed));
    }

    #[test]
    fn given_failed_job_when_outcome_set_should_include_reason() {
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = JobState::Running;
        job.mark_failed("reason".to_string()).unwrap();

        assert_eq!(job.outcome_reason, Some("reason".to_string()));
    }

    #[test]
    fn given_successful_job_when_marked_should_clear_outcome_reason() {
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.state = JobState::Running;
        job.mark_failed("reason".to_string()).unwrap();
        job.state = JobState::Running;
        job.mark_succeeded().unwrap();

        assert_eq!(job.outcome, Some(JobOutcome::Success));
        assert_eq!(job.outcome_reason, None);
    }

    #[test]
    fn given_canceled_job_when_marked_should_set_canceled_outcome() {
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        job.mark_canceled(Some("cancel".to_string()));

        assert_eq!(job.outcome, Some(JobOutcome::Canceled));
        assert_eq!(job.outcome_reason, Some("cancel".to_string()));
    }

    #[test]
    fn given_job_when_state_changes_should_update_updated_at() {
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        let before = job.updated_at;
        job.state = JobState::Running;
        job.mark_succeeded().unwrap();

        assert!(job.updated_at.as_inner() >= before.as_inner());
    }

    #[test]
    fn given_job_when_mark_failed_should_update_updated_at() {
        let mut job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        let before = job.updated_at;
        job.state = JobState::Running;
        job.mark_failed("err".to_string()).unwrap();

        assert!(job.updated_at.as_inner() >= before.as_inner());
    }
}
