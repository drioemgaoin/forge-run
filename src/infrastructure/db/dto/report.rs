use crate::domain::entities::job::JobOutcome;
use crate::domain::entities::report::{EventSnapshot, Report};
use crate::domain::value_objects::ids::JobId;
use crate::domain::value_objects::timestamps::Timestamp;
use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportRow {
    pub job_id: uuid::Uuid,
    pub outcome: String,
    pub outcome_reason: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: OffsetDateTime,
    pub duration_ms: i64,
    pub created_at: OffsetDateTime,
}

impl ReportRow {
    pub fn from_report(report: &Report) -> Self {
        Self {
            job_id: report.job_id.0,
            outcome: outcome_to_str(report.outcome).to_string(),
            outcome_reason: report.outcome_reason.clone(),
            started_at: report.started_at.as_inner(),
            finished_at: report.finished_at.as_inner(),
            duration_ms: report.duration_ms,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    pub fn into_report(self) -> Report {
        self.into_report_with_events(Vec::new())
    }

    pub fn into_report_with_events(self, events: Vec<EventSnapshot>) -> Report {
        Report {
            job_id: JobId(self.job_id),
            outcome: outcome_from_str(&self.outcome),
            outcome_reason: self.outcome_reason,
            started_at: Timestamp::from(self.started_at),
            finished_at: Timestamp::from(self.finished_at),
            duration_ms: self.duration_ms,
            events,
        }
    }
}

fn outcome_to_str(outcome: JobOutcome) -> &'static str {
    match outcome {
        JobOutcome::Success => "success",
        JobOutcome::Failed => "failed",
        JobOutcome::Canceled => "canceled",
    }
}

fn outcome_from_str(value: &str) -> JobOutcome {
    match value {
        "success" => JobOutcome::Success,
        "failed" => JobOutcome::Failed,
        _ => JobOutcome::Canceled,
    }
}

#[cfg(test)]
mod tests {
    use super::{ReportRow, outcome_from_str};
    use crate::domain::entities::job::JobOutcome;
    use crate::domain::entities::report::Report;
    use crate::domain::value_objects::ids::JobId;
    use crate::domain::value_objects::timestamps::Timestamp;
    use time::OffsetDateTime;

    fn sample_report() -> Report {
        let now = Timestamp::now_utc();
        Report {
            job_id: JobId::new(),
            outcome: JobOutcome::Success,
            outcome_reason: None,
            started_at: now,
            finished_at: now,
            duration_ms: 0,
            events: Vec::new(),
        }
    }

    #[test]
    fn given_report_when_from_report_should_map_fields() {
        let report = sample_report();

        let row = ReportRow::from_report(&report);

        assert_eq!(row.job_id, report.job_id.0);
        assert_eq!(row.outcome, "success");
        assert_eq!(row.outcome_reason, report.outcome_reason);
        assert_eq!(row.started_at, report.started_at.as_inner());
        assert_eq!(row.finished_at, report.finished_at.as_inner());
        assert_eq!(row.duration_ms, report.duration_ms);
    }

    #[test]
    fn given_report_row_when_into_report_should_map_fields() {
        let now = OffsetDateTime::now_utc();
        let row = ReportRow {
            job_id: uuid::Uuid::new_v4(),
            outcome: "failed".to_string(),
            outcome_reason: Some("boom".to_string()),
            started_at: now,
            finished_at: now,
            duration_ms: 42,
            created_at: now,
        };

        let report = row.clone().into_report();

        assert_eq!(report.job_id.0, row.job_id);
        assert_eq!(report.outcome, JobOutcome::Failed);
        assert_eq!(report.outcome_reason, Some("boom".to_string()));
        assert_eq!(report.started_at, Timestamp::from(row.started_at));
        assert_eq!(report.finished_at, Timestamp::from(row.finished_at));
        assert_eq!(report.duration_ms, 42);
    }

    #[test]
    fn given_unknown_outcome_when_mapped_should_default_to_canceled() {
        assert_eq!(outcome_from_str("unknown"), JobOutcome::Canceled);
    }
}
