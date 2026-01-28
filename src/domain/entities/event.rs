use crate::domain::entities::job::JobState;
use crate::domain::value_objects::ids::{EventId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::domain::workflows::state_machine::TransitionError;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
pub struct Event {
    pub id: EventId,
    pub job_id: JobId,
    pub event_name: EventName,
    pub prev_state: JobState,
    pub next_state: JobState,
    pub timestamp: Timestamp,
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

impl EventName {
    pub fn is_start(self) -> bool {
        matches!(self, EventName::JobStarted)
    }

    pub fn is_final(self) -> bool {
        matches!(
            self,
            EventName::JobSucceeded | EventName::JobFailed | EventName::JobCanceled
        )
    }
}

impl Event {
    const TRANSITIONS: [((JobState, JobState), EventName); 10] = [
        ((JobState::Created, JobState::Queued), EventName::JobQueued),
        (
            (JobState::Created, JobState::Canceled),
            EventName::JobCanceled,
        ),
        (
            (JobState::Queued, JobState::Assigned),
            EventName::JobAssigned,
        ),
        (
            (JobState::Queued, JobState::Canceled),
            EventName::JobCanceled,
        ),
        (
            (JobState::Assigned, JobState::Running),
            EventName::JobStarted,
        ),
        (
            (JobState::Assigned, JobState::Canceled),
            EventName::JobCanceled,
        ),
        (
            (JobState::Running, JobState::Succeeded),
            EventName::JobSucceeded,
        ),
        ((JobState::Running, JobState::Failed), EventName::JobFailed),
        (
            (JobState::Running, JobState::Canceled),
            EventName::JobCanceled,
        ),
        ((JobState::Failed, JobState::Queued), EventName::JobQueued),
    ];

    pub fn from_transition(
        id: EventId,
        job_id: JobId,
        prev_state: JobState,
        next_state: JobState,
        timestamp: Timestamp,
    ) -> Result<Self, TransitionError> {
        let event_name = Self::TRANSITIONS
            .iter()
            .find(|(pair, _)| pair.0 == prev_state && pair.1 == next_state)
            .map(|(_, name)| *name)
            .ok_or(TransitionError::Forbidden)?;
        Ok(Self {
            id,
            job_id,
            event_name,
            prev_state,
            next_state,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workflows::state_machine::TransitionError;

    #[test]
    fn given_created_to_queued_transition_when_from_transition_called_should_emit_job_queued() {
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Created,
            JobState::Queued,
            Timestamp::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobQueued);
    }

    #[test]
    fn given_assigned_to_running_transition_when_from_transition_called_should_emit_job_started() {
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Assigned,
            JobState::Running,
            Timestamp::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobStarted);
    }

    #[test]
    fn given_running_to_succeeded_transition_when_from_transition_called_should_emit_job_succeeded()
    {
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Running,
            JobState::Succeeded,
            Timestamp::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobSucceeded);
    }

    #[test]
    fn given_running_to_failed_transition_when_from_transition_called_should_emit_job_failed() {
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Running,
            JobState::Failed,
            Timestamp::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobFailed);
    }

    #[test]
    fn given_running_to_canceled_transition_when_from_transition_called_should_emit_job_canceled() {
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Running,
            JobState::Canceled,
            Timestamp::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobCanceled);
    }

    #[test]
    fn given_failed_to_queued_transition_when_from_transition_called_should_emit_job_queued() {
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Failed,
            JobState::Queued,
            Timestamp::now_utc(),
        );
        let event = result.expect("event should be created");
        assert_eq!(event.event_name, EventName::JobQueued);
    }

    #[test]
    fn given_transition_when_from_transition_called_should_set_ids_and_states() {
        let now = Timestamp::now_utc();
        let event_id = EventId::new();
        let job_id = JobId::new();
        let event =
            Event::from_transition(event_id, job_id, JobState::Queued, JobState::Assigned, now)
                .expect("event should be created");

        assert_eq!(event.id, event_id);
        assert_eq!(event.job_id, job_id);
        assert_eq!(event.prev_state, JobState::Queued);
        assert_eq!(event.next_state, JobState::Assigned);
    }

    #[test]
    fn given_transition_when_from_transition_called_should_set_timestamp() {
        let now = Timestamp::now_utc();
        let event = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Created,
            JobState::Queued,
            now,
        )
        .expect("event should be created");

        assert_eq!(event.timestamp, now);
    }

    #[test]
    fn given_forbidden_transition_when_from_transition_called_should_not_use_created_as_default() {
        let now = Timestamp::now_utc();
        let result = Event::from_transition(
            EventId::new(),
            JobId::new(),
            JobState::Created,
            JobState::Running,
            now,
        );
        assert_eq!(result, Err(TransitionError::Forbidden));
    }
}
