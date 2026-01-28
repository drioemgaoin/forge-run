use crate::domain::entities::job::JobState;
use crate::domain::workflows::state_machine::TransitionError;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, PartialEq)]
pub struct Event {
    pub id: u64,
    pub job_id: u64,
    pub event_name: EventName,
    pub prev_state: JobState,
    pub next_state: JobState,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventName {
    JobCreated,
    JobQueued,
    JobAssigned,
    JobStarted,
    JobSucceeded,
    JobFailed,
    JobCanceled,
}

impl Event {
    const TRANSITIONS: [((JobState, JobState), EventName); 10] = [
        ((JobState::Created, JobState::Queued), EventName::JobQueued),
        ((JobState::Created, JobState::Canceled), EventName::JobCanceled),
        ((JobState::Queued, JobState::Assigned), EventName::JobAssigned),
        ((JobState::Queued, JobState::Canceled), EventName::JobCanceled),
        ((JobState::Assigned, JobState::Running), EventName::JobStarted),
        ((JobState::Assigned, JobState::Canceled), EventName::JobCanceled),
        ((JobState::Running, JobState::Succeeded), EventName::JobSucceeded),
        ((JobState::Running, JobState::Failed), EventName::JobFailed),
        ((JobState::Running, JobState::Canceled), EventName::JobCanceled),
        ((JobState::Failed, JobState::Queued), EventName::JobQueued),
    ];

    fn new(
        id: u64,
        job_id: u64,
        event_name: EventName,
        prev_state: JobState,
        next_state: JobState,
        timestamp: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            job_id,
            event_name,
            prev_state,
            next_state,
            timestamp,
        }
    }

    pub fn from_transition(
        id: u64,
        job_id: u64,
        prev_state: JobState,
        next_state: JobState,
        timestamp: OffsetDateTime,
    ) -> Result<Self, TransitionError> {
        let event_name = Self::TRANSITIONS
            .iter()
            .find(|(pair, _)| pair.0 == prev_state && pair.1 == next_state)
            .map(|(_, name)| *name)
            .ok_or(TransitionError::Forbidden)?;
        Ok(Self::new(
            id,
            job_id,
            event_name,
            prev_state,
            next_state,
            timestamp,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workflows::state_machine::TransitionError;

    #[test]
    fn given_created_to_queued_transition_when_from_transition_called_should_emit_job_queued() {
        let result = Event::from_transition(
            1,
            100,
            JobState::Created,
            JobState::Queued,
            OffsetDateTime::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobQueued);
    }

    #[test]
    fn given_assigned_to_running_transition_when_from_transition_called_should_emit_job_started() {
        let result = Event::from_transition(
            1,
            100,
            JobState::Assigned,
            JobState::Running,
            OffsetDateTime::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobStarted);
    }

    #[test]
    fn given_running_to_succeeded_transition_when_from_transition_called_should_emit_job_succeeded()
    {
        let result = Event::from_transition(
            1,
            100,
            JobState::Running,
            JobState::Succeeded,
            OffsetDateTime::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobSucceeded);
    }

    #[test]
    fn given_running_to_failed_transition_when_from_transition_called_should_emit_job_failed() {
        let result = Event::from_transition(
            1,
            100,
            JobState::Running,
            JobState::Failed,
            OffsetDateTime::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobFailed);
    }

    #[test]
    fn given_running_to_canceled_transition_when_from_transition_called_should_emit_job_canceled() {
        let result = Event::from_transition(
            1,
            100,
            JobState::Running,
            JobState::Canceled,
            OffsetDateTime::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobCanceled);
    }

    #[test]
    fn given_failed_to_queued_transition_when_from_transition_called_should_emit_job_queued() {
        let result = Event::from_transition(
            1,
            100,
            JobState::Failed,
            JobState::Queued,
            OffsetDateTime::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobQueued);
    }

    #[test]
    fn given_transition_when_from_transition_called_should_set_ids_and_states() {
        let now = OffsetDateTime::now_utc();
        let event = Event::from_transition(10, 20, JobState::Queued, JobState::Assigned, now)
            .expect("event should be created");

        assert_eq!(event.id, 10);
        assert_eq!(event.job_id, 20);
        assert_eq!(event.prev_state, JobState::Queued);
        assert_eq!(event.next_state, JobState::Assigned);
    }

    #[test]
    fn given_transition_when_from_transition_called_should_set_timestamp() {
        let now = OffsetDateTime::now_utc();
        let event = Event::from_transition(1, 1, JobState::Created, JobState::Queued, now)
            .expect("event should be created");

        assert_eq!(event.timestamp, now);
    }

    #[test]
    fn given_forbidden_transition_when_from_transition_called_should_not_use_created_as_default() {
        let now = OffsetDateTime::now_utc();
        let result = Event::from_transition(1, 1, JobState::Created, JobState::Running, now);
        assert_eq!(result, Err(TransitionError::Forbidden));
    }
}
