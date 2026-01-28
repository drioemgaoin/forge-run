use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    Instant,
    Deferred,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobOutcome {
    Success,
    Failed,
    Canceled,
}

pub struct Job {
    pub id: u64,
    pub client_id: u64,
    pub job_type: JobType,
    pub state: JobState,
    pub attempt: u8,
    pub outcome: Option<JobOutcome>,
    pub outcome_reason: Option<String>,
    pub execution_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub callback_url: Option<String>,
    pub working_kind: Option<String>,
}

impl Job {
    fn new(
        id: u64,
        client_id: u64,
        job_type: JobType,
        execution_at: Option<OffsetDateTime>,
        callback_url: Option<String>,
        working_kind: Option<String>,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            client_id,
            job_type,
            state: JobState::Created,
            attempt: 0,
            outcome: None,
            outcome_reason: None,
            execution_at,
            created_at: now,
            updated_at: now,
            callback_url,
            working_kind,
        }
    }

    pub fn new_instant(
        id: u64,
        client_id: u64,
        callback_url: Option<String>,
        working_kind: Option<String>,
        now: OffsetDateTime,
    ) -> Self {
        Self::new(
            id,
            client_id,
            JobType::Instant,
            None,
            callback_url,
            working_kind,
            now,
        )
    }

    pub fn new_deferred(
        id: u64,
        client_id: u64,
        execution_at: OffsetDateTime,
        callback_url: Option<String>,
        working_kind: Option<String>,
        now: OffsetDateTime,
    ) -> Self {
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

    pub fn mark_succeeded(&mut self, now: OffsetDateTime) {
        self.state = JobState::Succeeded;
        self.outcome = Some(JobOutcome::Success);
        self.outcome_reason = None;
        self.updated_at = now;
    }

    pub fn mark_failed(&mut self, reason: String, now: OffsetDateTime) {
        self.state = JobState::Failed;
        self.outcome = Some(JobOutcome::Failed);
        self.outcome_reason = Some(reason);
        self.updated_at = now;
    }

    pub fn mark_canceled(&mut self, reason: Option<String>, now: OffsetDateTime) {
        self.state = JobState::Canceled;
        self.outcome = Some(JobOutcome::Canceled);
        self.outcome_reason = reason;
        self.updated_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_new_job_when_created_should_have_initial_state_and_attempt_zero() {
        let now = OffsetDateTime::now_utc();
        let job = Job::new_instant(1, 1, None, None, now);

        assert_eq!(job.state, JobState::Created);
        assert_eq!(job.attempt, 0);
        assert_eq!(job.created_at, now);
        assert_eq!(job.updated_at, now);
    }

    #[test]
    fn given_deferred_job_when_created_should_have_execution_at_set() {
        let now = OffsetDateTime::now_utc();
        let execution_at = now;
        let job = Job::new_deferred(1, 1, execution_at, None, None, now);

        assert_eq!(job.execution_at, Some(execution_at));
    }

    #[test]
    fn given_instant_job_when_created_should_have_no_execution_at() {
        let now = OffsetDateTime::now_utc();
        let job = Job::new_instant(1, 1, None, None, now);

        assert_eq!(job.execution_at, None);
    }

    #[test]
    fn given_deferred_job_when_created_should_have_job_type_deferred() {
        let now = OffsetDateTime::now_utc();
        let execution_at = now;
        let job = Job::new_deferred(1, 1, execution_at, None, None, now);

        assert_eq!(job.job_type, JobType::Deferred);
    }

    #[test]
    fn given_job_with_no_callback_when_created_should_have_none_callback_url() {
        let now = OffsetDateTime::now_utc();
        let job = Job::new_instant(1, 1, None, None, now);

        assert_eq!(job.callback_url, None);
    }

    #[test]
    fn given_failed_job_when_outcome_set_should_be_failed() {
        let now = OffsetDateTime::now_utc();
        let mut job = Job::new_instant(1, 1, None, None, now);
        job.mark_failed("error".to_string(), now);

        assert_eq!(job.state, JobState::Failed);
        assert_eq!(job.outcome, Some(JobOutcome::Failed));
    }

    #[test]
    fn given_failed_job_when_outcome_set_should_include_reason() {
        let now = OffsetDateTime::now_utc();
        let mut job = Job::new_instant(1, 1, None, None, now);
        job.mark_failed("reason".to_string(), now);

        assert_eq!(job.outcome_reason, Some("reason".to_string()));
    }

    #[test]
    fn given_successful_job_when_marked_should_clear_outcome_reason() {
        let now = OffsetDateTime::now_utc();
        let mut job = Job::new_instant(1, 1, None, None, now);
        job.mark_failed("reason".to_string(), now);
        job.mark_succeeded(now);

        assert_eq!(job.outcome, Some(JobOutcome::Success));
        assert_eq!(job.outcome_reason, None);
    }

    #[test]
    fn given_canceled_job_when_marked_should_set_canceled_outcome() {
        let now = OffsetDateTime::now_utc();
        let mut job = Job::new_instant(1, 1, None, None, now);
        job.mark_canceled(Some("cancel".to_string()), now);

        assert_eq!(job.outcome, Some(JobOutcome::Canceled));
        assert_eq!(job.outcome_reason, Some("cancel".to_string()));
    }

    #[test]
    fn given_job_when_state_changes_should_update_updated_at() {
        let now = OffsetDateTime::now_utc();
        let later = now + time::Duration::seconds(1);
        let mut job = Job::new_instant(1, 1, None, None, now);
        job.mark_succeeded(later);

        assert_eq!(job.updated_at, later);
    }

    #[test]
    fn given_job_when_mark_failed_should_update_updated_at() {
        let now = OffsetDateTime::now_utc();
        let later = now + time::Duration::seconds(1);
        let mut job = Job::new_instant(1, 1, None, None, now);
        job.mark_failed("err".to_string(), later);

        assert_eq!(job.updated_at, later);
    }
}
