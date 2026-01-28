use crate::domain::entities::event::EventName;
use crate::domain::entities::job::{JobOutcome, JobState};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportError {
    MissingStartedEvent,
    MissingFinishedEvent,
    StartedEventNotFirst,
    FinishedEventNotLast,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSnapshot {
    pub event_name: EventName,
    pub prev_state: JobState,
    pub next_state: JobState,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub job_id: u64,
    pub outcome: JobOutcome,
    pub outcome_reason: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: OffsetDateTime,
    pub duration_ms: i64,
    pub events: Vec<EventSnapshot>,
}

impl Report {
    pub fn from_events(
        job_id: u64,
        outcome: JobOutcome,
        outcome_reason: Option<String>,
        mut events: Vec<EventSnapshot>,
    ) -> Result<Self, ReportError> {
        if events.is_empty() {
            return Err(ReportError::MissingStartedEvent);
        }

        events.sort_by_key(|e| e.timestamp);

        let started_at = events
            .iter()
            .find(|e| e.event_name.is_start())
            .map(|e| e.timestamp)
            .ok_or(ReportError::MissingStartedEvent)?;

        let finished_at = events
            .iter()
            .rev()
            .find(|e| e.event_name.is_final())
            .map(|e| e.timestamp)
            .ok_or(ReportError::MissingFinishedEvent)?;

        let first_ts = events
            .first()
            .map(|e| e.timestamp)
            .ok_or(ReportError::MissingStartedEvent)?;
        let last_ts = events
            .last()
            .map(|e| e.timestamp)
            .ok_or(ReportError::MissingFinishedEvent)?;

        if started_at != first_ts {
            return Err(ReportError::StartedEventNotFirst);
        }
        if finished_at != last_ts {
            return Err(ReportError::FinishedEventNotLast);
        }

        let duration = finished_at - started_at;

        Ok(Self {
            job_id,
            outcome,
            outcome_reason,
            started_at,
            finished_at,
            duration_ms: duration.whole_milliseconds() as i64,
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_successful_events_when_from_events_called_should_set_outcome_and_times() {
        let job_id = 1;
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = OffsetDateTime::now_utc();
        let t1 = t0 + time::Duration::seconds(10);

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t1,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(result.outcome, JobOutcome::Success);
        assert_eq!(result.started_at, t0);
        assert_eq!(result.finished_at, t1);
    }

    #[test]
    fn given_failed_events_when_from_events_called_should_set_failed_outcome() {
        let job_id = 1;
        let outcome = JobOutcome::Failed;
        let outcome_reason = Some("Some error".to_string());
        let t0 = OffsetDateTime::now_utc();
        let t1 = t0 + time::Duration::seconds(10);

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobFailed,
                prev_state: JobState::Running,
                next_state: JobState::Failed,
                timestamp: t1,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(result.outcome, JobOutcome::Failed);
        assert_eq!(result.outcome_reason, Some("Some error".to_string()));
    }

    #[test]
    fn given_canceled_events_when_from_events_called_should_set_canceled_outcome() {
        let job_id = 1;
        let outcome = JobOutcome::Canceled;
        let outcome_reason = Some("Cancel by client".to_string());
        let t0 = OffsetDateTime::now_utc();
        let t1 = t0 + time::Duration::seconds(10);

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobCanceled,
                prev_state: JobState::Running,
                next_state: JobState::Canceled,
                timestamp: t1,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(result.outcome, JobOutcome::Canceled);
        assert_eq!(result.outcome_reason, Some("Cancel by client".to_string()));
    }

    #[test]
    fn given_missing_started_event_when_from_events_called_should_return_error() {
        let job_id = 1;
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = OffsetDateTime::now_utc();

        let events = vec![EventSnapshot {
            event_name: EventName::JobSucceeded,
            prev_state: JobState::Running,
            next_state: JobState::Succeeded,
            timestamp: t0,
        }];

        let result = Report::from_events(job_id, outcome, outcome_reason, events);
        assert_eq!(result, Err(ReportError::MissingStartedEvent));
    }

    #[test]
    fn given_missing_finished_event_when_from_events_called_should_return_error() {
        let job_id = 1;
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = OffsetDateTime::now_utc();

        let events = vec![EventSnapshot {
            event_name: EventName::JobStarted,
            prev_state: JobState::Assigned,
            next_state: JobState::Running,
            timestamp: t0,
        }];

        let result = Report::from_events(job_id, outcome, outcome_reason, events);
        assert_eq!(result, Err(ReportError::MissingFinishedEvent));
    }

    #[test]
    fn given_events_when_from_events_called_should_compute_duration_ms() {
        let job_id = 1;
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = OffsetDateTime::now_utc();
        let t1 = t0 + time::Duration::milliseconds(1500);

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t1,
            },
        ];

        let report = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(report.duration_ms, 1500);
    }
}
