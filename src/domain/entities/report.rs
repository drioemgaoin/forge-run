use crate::domain::entities::event::EventName;
use crate::domain::entities::job::{JobOutcome, JobState};
use crate::domain::value_objects::ids::JobId;
use crate::domain::value_objects::timestamps::Timestamp;
use crate::domain::workflows::state_machine::JobStateMachine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportError {
    MissingStartedEvent,
    MissingFinishedEvent,
    StartedEventNotFirst,
    FinishedEventNotLast,
    OutcomeMismatch,
    InvalidTransition,
    NonContiguousEvents,
    MultipleStartedEvents,
    MultipleFinishedEvents,
    MultipleCreatedEvents,
    CreatedEventNotFirst,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSnapshot {
    pub event_name: EventName,
    pub prev_state: JobState,
    pub next_state: JobState,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub job_id: JobId,
    pub outcome: JobOutcome,
    pub outcome_reason: Option<String>,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub duration_ms: i64,
    pub events: Vec<EventSnapshot>,
}

impl Report {
    pub fn from_events(
        job_id: JobId,
        outcome: JobOutcome,
        outcome_reason: Option<String>,
        mut events: Vec<EventSnapshot>,
    ) -> Result<Self, ReportError> {
        if events.is_empty() {
            return Err(ReportError::MissingStartedEvent);
        }

        // Sort events by timestamp
        events.sort_by_key(|e| e.timestamp.as_inner());

        // Find the started_at timestamp
        let started_events: Vec<&EventSnapshot> =
            events.iter().filter(|e| e.event_name.is_start()).collect();
        if started_events.is_empty() {
            return Err(ReportError::MissingStartedEvent);
        }
        if started_events.len() > 1 {
            return Err(ReportError::MultipleStartedEvents);
        }
        let started_at = started_events[0].timestamp;

        // Find the finished_at timestamp
        let finished_events: Vec<&EventSnapshot> =
            events.iter().filter(|e| e.event_name.is_final()).collect();
        if finished_events.is_empty() {
            return Err(ReportError::MissingFinishedEvent);
        }
        if finished_events.len() > 1 {
            return Err(ReportError::MultipleFinishedEvents);
        }
        let finished_at = finished_events[0].timestamp;

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

        let final_event = finished_events[0];
        let matches_outcome = matches!(
            (final_event.event_name, outcome),
            (EventName::JobSucceeded, JobOutcome::Success)
                | (EventName::JobFailed, JobOutcome::Failed)
                | (EventName::JobCanceled, JobOutcome::Canceled)
        );
        if !matches_outcome {
            return Err(ReportError::OutcomeMismatch);
        }

        for event in &events {
            if event.event_name == EventName::JobCreated {
                continue;
            }
            if !JobStateMachine::can_transition(event.prev_state, event.next_state) {
                return Err(ReportError::InvalidTransition);
            }
        }

        for pair in events.windows(2) {
            let prev = &pair[0];
            let next = &pair[1];
            if prev.event_name == EventName::JobCreated {
                continue;
            }
            if prev.next_state != next.prev_state {
                return Err(ReportError::NonContiguousEvents);
            }
        }

        let duration = finished_at.as_inner() - started_at.as_inner();

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
        let job_id = JobId::new();
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));
        let t2 = Timestamp::from(t0.as_inner() + time::Duration::seconds(10));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t2,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(result.outcome, JobOutcome::Success);
        assert_eq!(result.started_at, t0);
        assert_eq!(result.finished_at, t2);
    }

    #[test]
    fn given_failed_events_when_from_events_called_should_set_failed_outcome() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Failed;
        let outcome_reason = Some("Some error".to_string());
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));
        let t2 = Timestamp::from(t0.as_inner() + time::Duration::seconds(10));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
            EventSnapshot {
                event_name: EventName::JobFailed,
                prev_state: JobState::Running,
                next_state: JobState::Failed,
                timestamp: t2,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(result.outcome, JobOutcome::Failed);
        assert_eq!(result.outcome_reason, Some("Some error".to_string()));
    }

    #[test]
    fn given_canceled_events_when_from_events_called_should_set_canceled_outcome() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Canceled;
        let outcome_reason = Some("Cancel by client".to_string());
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));
        let t2 = Timestamp::from(t0.as_inner() + time::Duration::seconds(10));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
            EventSnapshot {
                event_name: EventName::JobCanceled,
                prev_state: JobState::Running,
                next_state: JobState::Canceled,
                timestamp: t2,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(result.outcome, JobOutcome::Canceled);
        assert_eq!(result.outcome_reason, Some("Cancel by client".to_string()));
    }

    #[test]
    fn given_missing_started_event_when_from_events_called_should_return_error() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = Timestamp::now_utc();

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
                timestamp: t0,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events);
        assert_eq!(result, Err(ReportError::MissingStartedEvent));
    }

    #[test]
    fn given_missing_finished_event_when_from_events_called_should_return_error() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events);
        assert_eq!(result, Err(ReportError::MissingFinishedEvent));
    }

    #[test]
    fn given_events_when_from_events_called_should_compute_duration_ms() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));
        let t2 = Timestamp::from(t0.as_inner() + time::Duration::milliseconds(2500));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t2,
            },
        ];

        let report = Report::from_events(job_id, outcome, outcome_reason, events).unwrap();
        assert_eq!(report.duration_ms, 2500);
    }

    #[test]
    fn given_outcome_mismatch_when_from_events_called_should_return_error() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));
        let t2 = Timestamp::from(t0.as_inner() + time::Duration::seconds(10));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
            EventSnapshot {
                event_name: EventName::JobFailed,
                prev_state: JobState::Running,
                next_state: JobState::Failed,
                timestamp: t2,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events);
        assert_eq!(result, Err(ReportError::OutcomeMismatch));
    }

    #[test]
    fn given_non_contiguous_events_when_from_events_called_should_return_error() {
        let job_id = JobId::new();
        let outcome = JobOutcome::Success;
        let outcome_reason = None;
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + time::Duration::seconds(1));
        let t2 = Timestamp::from(t0.as_inner() + time::Duration::seconds(5));
        let t3 = Timestamp::from(t0.as_inner() + time::Duration::seconds(10));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobStarted,
                prev_state: JobState::Assigned,
                next_state: JobState::Running,
                timestamp: t1,
            },
            EventSnapshot {
                event_name: EventName::JobQueued,
                prev_state: JobState::Created,
                next_state: JobState::Queued,
                timestamp: t2,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t3,
            },
        ];

        let result = Report::from_events(job_id, outcome, outcome_reason, events);
        assert_eq!(result, Err(ReportError::NonContiguousEvents));
    }
}
