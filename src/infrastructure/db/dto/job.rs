use crate::domain::entities::job::{Job, JobOutcome, JobState, JobType};
use crate::domain::value_objects::ids::{ClientId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobRow {
    pub id: uuid::Uuid,
    pub client_id: uuid::Uuid,
    pub job_type: String,
    pub state: String,
    pub attempt: i32,
    pub outcome: Option<String>,
    pub outcome_reason: Option<String>,
    pub executed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub callback_url: Option<String>,
    pub work_kind: String,
}

impl JobRow {
    pub fn from_job(job: &Job) -> Self {
        Self {
            id: job.id.0,
            client_id: job.client_id.0,
            job_type: job.job_type.as_str().to_string(),
            state: job.state.as_str().to_string(),
            attempt: i32::from(job.attempt),
            outcome: job.outcome.map(|o| o.as_str().to_string()),
            outcome_reason: job.outcome_reason.clone(),
            executed_at: job.executed_at.map(|t| t.as_inner()),
            created_at: job.created_at.as_inner(),
            updated_at: job.updated_at.as_inner(),
            callback_url: job.callback_url.clone(),
            work_kind: job.working_kind.clone().unwrap_or_default(),
        }
    }

    pub fn into_job(self) -> Job {
        Job {
            id: JobId(self.id),
            client_id: ClientId(self.client_id),
            job_type: match self.job_type.as_str() {
                "instant" => JobType::Instant,
                _ => JobType::Deferred,
            },
            state: match self.state.as_str() {
                "created" => JobState::Created,
                "queued" => JobState::Queued,
                "assigned" => JobState::Assigned,
                "running" => JobState::Running,
                "succeeded" => JobState::Succeeded,
                "failed" => JobState::Failed,
                _ => JobState::Canceled,
            },
            attempt: self.attempt as u8,
            outcome: match self.outcome.as_deref() {
                Some("success") => Some(JobOutcome::Success),
                Some("failed") => Some(JobOutcome::Failed),
                Some("canceled") => Some(JobOutcome::Canceled),
                _ => None,
            },
            outcome_reason: self.outcome_reason,
            executed_at: self.executed_at.map(Timestamp::from),
            created_at: Timestamp::from(self.created_at),
            updated_at: Timestamp::from(self.updated_at),
            callback_url: self.callback_url,
            working_kind: Some(self.work_kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JobRow;
    use crate::domain::entities::job::{Job, JobOutcome, JobState, JobType};
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use time::OffsetDateTime;

    fn sample_job() -> Job {
        Job::new_instant(
            JobId::new(),
            ClientId::new(),
            Some("https://example.com/callback".to_string()),
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap()
    }

    #[test]
    fn given_job_when_from_job_should_map_fields() {
        let mut job = sample_job();
        job.state = JobState::Running;
        job.attempt = 2;
        job.outcome = Some(JobOutcome::Success);
        job.outcome_reason = Some("ok".to_string());
        job.executed_at = Some(Timestamp::now_utc());

        let row = JobRow::from_job(&job);

        assert_eq!(row.id, job.id.0);
        assert_eq!(row.client_id, job.client_id.0);
        assert_eq!(row.job_type, JobType::Instant.as_str());
        assert_eq!(row.state, JobState::Running.as_str());
        assert_eq!(row.attempt, 2);
        assert_eq!(row.outcome.as_deref(), Some("success"));
        assert_eq!(row.outcome_reason.as_deref(), Some("ok"));
        assert_eq!(row.executed_at, job.executed_at.map(|t| t.as_inner()));
        assert_eq!(row.created_at, job.created_at.as_inner());
        assert_eq!(row.updated_at, job.updated_at.as_inner());
        assert_eq!(row.callback_url, job.callback_url);
        assert_eq!(row.work_kind, "SUCCESS_FAST");
    }

    #[test]
    fn given_job_row_when_into_job_should_map_fields() {
        let now = OffsetDateTime::now_utc();
        let row = JobRow {
            id: uuid::Uuid::new_v4(),
            client_id: uuid::Uuid::new_v4(),
            job_type: "deferred".to_string(),
            state: "queued".to_string(),
            attempt: 3,
            outcome: Some("failed".to_string()),
            outcome_reason: Some("boom".to_string()),
            executed_at: Some(now),
            created_at: now,
            updated_at: now,
            callback_url: Some("https://example.com/callback".to_string()),
            work_kind: "PAYLOAD_SMALL".to_string(),
        };

        let job = row.clone().into_job();

        assert_eq!(job.id.0, row.id);
        assert_eq!(job.client_id.0, row.client_id);
        assert_eq!(job.job_type, JobType::Deferred);
        assert_eq!(job.state, JobState::Queued);
        assert_eq!(job.attempt, 3);
        assert_eq!(job.outcome, Some(JobOutcome::Failed));
        assert_eq!(job.outcome_reason, Some("boom".to_string()));
        assert_eq!(job.executed_at, row.executed_at.map(Timestamp::from));
        assert_eq!(job.created_at, Timestamp::from(row.created_at));
        assert_eq!(job.updated_at, Timestamp::from(row.updated_at));
        assert_eq!(job.callback_url, row.callback_url);
        assert_eq!(job.working_kind, Some("PAYLOAD_SMALL".to_string()));
    }

    #[test]
    fn given_job_row_with_unknown_values_when_into_job_should_map_defaults() {
        let now = OffsetDateTime::now_utc();
        let row = JobRow {
            id: uuid::Uuid::new_v4(),
            client_id: uuid::Uuid::new_v4(),
            job_type: "unknown".to_string(),
            state: "unknown".to_string(),
            attempt: 0,
            outcome: Some("unknown".to_string()),
            outcome_reason: None,
            executed_at: None,
            created_at: now,
            updated_at: now,
            callback_url: None,
            work_kind: "SUCCESS_FAST".to_string(),
        };

        let job = row.into_job();

        assert_eq!(job.job_type, JobType::Deferred);
        assert_eq!(job.state, JobState::Canceled);
        assert_eq!(job.outcome, None);
    }
}
