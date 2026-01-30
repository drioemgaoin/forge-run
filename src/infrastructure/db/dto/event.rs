use crate::domain::entities::event::{Event, EventName};
use crate::domain::entities::job::JobState;
use crate::domain::value_objects::ids::{EventId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventRow {
    pub id: uuid::Uuid,
    pub job_id: uuid::Uuid,
    pub event_name: String,
    pub prev_state: String,
    pub next_state: String,
    pub occurred_at: OffsetDateTime,
}

impl EventRow {
    pub fn from_event(event: &Event) -> Self {
        Self {
            id: event.id.0,
            job_id: event.job_id.0,
            event_name: event_name_to_str(event.event_name).to_string(),
            prev_state: event.prev_state.as_str().to_string(),
            next_state: event.next_state.as_str().to_string(),
            occurred_at: event.timestamp.as_inner(),
        }
    }

    pub fn into_event(self) -> Event {
        Event {
            id: EventId(self.id),
            job_id: JobId(self.job_id),
            event_name: event_name_from_str(&self.event_name),
            prev_state: job_state_from_str(&self.prev_state),
            next_state: job_state_from_str(&self.next_state),
            timestamp: Timestamp::from(self.occurred_at),
        }
    }
}

fn event_name_to_str(name: EventName) -> &'static str {
    match name {
        EventName::JobCreated => "job_created",
        EventName::JobQueued => "job_queued",
        EventName::JobAssigned => "job_assigned",
        EventName::JobStarted => "job_started",
        EventName::JobSucceeded => "job_succeeded",
        EventName::JobFailed => "job_failed",
        EventName::JobCanceled => "job_canceled",
    }
}

fn event_name_from_str(value: &str) -> EventName {
    match value {
        "job_created" => EventName::JobCreated,
        "job_queued" => EventName::JobQueued,
        "job_assigned" => EventName::JobAssigned,
        "job_started" => EventName::JobStarted,
        "job_succeeded" => EventName::JobSucceeded,
        "job_failed" => EventName::JobFailed,
        _ => EventName::JobCanceled,
    }
}

fn job_state_from_str(value: &str) -> JobState {
    match value {
        "created" => JobState::Created,
        "queued" => JobState::Queued,
        "assigned" => JobState::Assigned,
        "running" => JobState::Running,
        "succeeded" => JobState::Succeeded,
        "failed" => JobState::Failed,
        _ => JobState::Canceled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::event::EventName;
    use crate::domain::entities::job::JobState;
    use crate::domain::value_objects::timestamps::Timestamp;
    use time::OffsetDateTime;

    #[test]
    fn given_event_when_from_event_should_map_fields() {
        let event = Event {
            id: EventId::new(),
            job_id: JobId::new(),
            event_name: EventName::JobQueued,
            prev_state: JobState::Created,
            next_state: JobState::Queued,
            timestamp: Timestamp::now_utc(),
        };

        let row = EventRow::from_event(&event);

        assert_eq!(row.id, event.id.0);
        assert_eq!(row.job_id, event.job_id.0);
        assert_eq!(row.event_name, "job_queued");
        assert_eq!(row.prev_state, "created");
        assert_eq!(row.next_state, "queued");
        assert_eq!(row.occurred_at, event.timestamp.as_inner());
    }

    #[test]
    fn given_event_row_when_into_event_should_map_fields() {
        let now = OffsetDateTime::now_utc();
        let row = EventRow {
            id: uuid::Uuid::new_v4(),
            job_id: uuid::Uuid::new_v4(),
            event_name: "job_failed".to_string(),
            prev_state: "running".to_string(),
            next_state: "failed".to_string(),
            occurred_at: now,
        };

        let event = row.clone().into_event();

        assert_eq!(event.id.0, row.id);
        assert_eq!(event.job_id.0, row.job_id);
        assert_eq!(event.event_name, EventName::JobFailed);
        assert_eq!(event.prev_state, JobState::Running);
        assert_eq!(event.next_state, JobState::Failed);
        assert_eq!(event.timestamp, Timestamp::from(row.occurred_at));
    }

    #[test]
    fn given_event_row_with_unknown_values_when_into_event_should_map_defaults() {
        let now = OffsetDateTime::now_utc();
        let row = EventRow {
            id: uuid::Uuid::new_v4(),
            job_id: uuid::Uuid::new_v4(),
            event_name: "unknown".to_string(),
            prev_state: "unknown".to_string(),
            next_state: "unknown".to_string(),
            occurred_at: now,
        };

        let event = row.into_event();

        assert_eq!(event.event_name, EventName::JobCanceled);
        assert_eq!(event.prev_state, JobState::Canceled);
        assert_eq!(event.next_state, JobState::Canceled);
    }
}
