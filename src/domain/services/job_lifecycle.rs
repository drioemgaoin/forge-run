use crate::domain::entities::event::Event;
use crate::domain::entities::job::{Job, JobState, JobValidationError};
use crate::domain::entities::report::{EventSnapshot, Report, ReportError};
use crate::domain::value_objects::ids::{ClientId, EventId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::domain::workflows::state_machine::{JobStateMachine, TransitionError};

// Job lifecycle orchestration (coordination across Job + Event + Report).
pub trait JobLifecycleService {
    fn create_instant(
        &self,
        job_id: JobId,
        client_id: ClientId,
        callback_url: Option<String>,
        work_kind: Option<String>,
    ) -> Result<(Job, Event), JobValidationError> {
        // Step 1: Create the Job (entity validates invariants).
        let job = Job::new_instant(job_id, client_id, callback_url, work_kind)?;

        // Step 2: Persist the Job.
        // TODO(persistence): persist job.

        // Step 3: Emit and persist a JobCreated event.
        let created_event = Event::new_created(EventId::new(), job.id, Timestamp::now_utc());
        // TODO(persistence): persist created_event.

        // Step 4: Return the created job and event.
        Ok((job, created_event))
    }

    fn create_deferred(
        &self,
        job_id: JobId,
        client_id: ClientId,
        execution_at: Timestamp,
        callback_url: Option<String>,
        work_kind: Option<String>,
    ) -> Result<(Job, Event), JobValidationError> {
        // Step 1: Create the Job (entity validates invariants).
        let job = Job::new_deferred(job_id, client_id, execution_at, callback_url, work_kind)?;

        // Step 2: Persist the Job.
        // TODO(persistence): persist job.

        // Step 3: Emit and persist a JobCreated event.
        let created_event = Event::new_created(EventId::new(), job.id, Timestamp::now_utc());
        // TODO(persistence): persist created_event.

        // Step 4: Return the created job and event.
        Ok((job, created_event))
    }

    fn transition(&self, job: &mut Job, next_state: JobState) -> Result<Event, TransitionError> {
        // Step 1: Capture prev_state from job.
        let prev_state = job.state;

        // Step 2: Validate transition (state machine).
        JobStateMachine::transition(prev_state, next_state)?;

        // Step 3: Mutate job state and outcome fields as needed (entity methods).
        match next_state {
            JobState::Succeeded => {
                let _ = job.mark_succeeded();
            }
            JobState::Failed => {
                // TODO: provide a real failure reason.
                let _ = job.mark_failed(String::new());
            }
            JobState::Canceled => {
                job.mark_canceled(None);
            }
            _ => {
                job.state = next_state;
                job.updated_at = Timestamp::now_utc();
            }
        }

        // Step 4: Create Event::from_transition(prev_state, next_state).
        let event = Event::from_transition(EventId::new(), job.id, prev_state, next_state)?;

        // Step 5: Persist job + event atomically.

        // Step 6: Return the created event.
        Ok(event)
    }

    fn finalize_report(
        &self,
        job_id: JobId,
        events: Vec<EventSnapshot>,
        outcome: crate::domain::entities::job::JobOutcome,
        outcome_reason: Option<String>,
    ) -> Result<Report, ReportError> {
        // Step 1: Build the report from the events (validation inside).
        let report = Report::from_events(job_id, outcome, outcome_reason, events)?;

        // Step 2: Persist the report.

        // Step 3: Return the report.
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::JobLifecycleService;
    use crate::domain::entities::event::EventName;
    use crate::domain::entities::job::{JobOutcome, JobState, JobValidationError};
    use crate::domain::entities::report::EventSnapshot;
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use time::Duration;

    struct DummyLifecycle;

    impl JobLifecycleService for DummyLifecycle {}

    #[test]
    fn given_valid_instant_job_when_created_should_return_job() {
        let lifecycle = DummyLifecycle;
        let (job, event) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .unwrap();

        assert_eq!(job.state, JobState::Created);
        assert_eq!(event.event_name, EventName::JobCreated);
        assert_eq!(event.job_id, job.id);
    }

    #[test]
    fn given_missing_work_kind_when_created_instant_should_return_error() {
        let lifecycle = DummyLifecycle;
        let result = lifecycle.create_instant(JobId::new(), ClientId::new(), None, None);

        assert!(matches!(result, Err(JobValidationError::MissingWorkKind)));
    }

    #[test]
    fn given_past_execution_at_when_created_deferred_should_return_error() {
        let lifecycle = DummyLifecycle;
        let now = Timestamp::now_utc();
        let execution_at = Timestamp::from(now.as_inner() - Duration::seconds(1));

        let result = lifecycle.create_deferred(
            JobId::new(),
            ClientId::new(),
            execution_at,
            None,
            Some("SUCCESS_FAST".to_string()),
        );

        assert!(matches!(result, Err(JobValidationError::ExecutionAtInPast)));
    }

    #[test]
    fn given_valid_transition_when_called_should_update_state_and_emit_event() {
        let lifecycle = DummyLifecycle;
        let (mut job, _) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .unwrap();

        let event = lifecycle.transition(&mut job, JobState::Queued).unwrap();

        assert_eq!(job.state, JobState::Queued);
        assert_eq!(event.event_name, EventName::JobQueued);
    }

    #[test]
    fn given_invalid_transition_when_called_should_return_error() {
        let lifecycle = DummyLifecycle;
        let (mut job, _) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .unwrap();

        let result = lifecycle.transition(&mut job, JobState::Running);

        assert_eq!(result, Err(crate::domain::workflows::state_machine::TransitionError::Forbidden));
    }

    #[test]
    fn given_running_to_succeeded_when_transitioned_should_set_outcome() {
        let lifecycle = DummyLifecycle;
        let (mut job, _) = lifecycle
            .create_instant(
                JobId::new(),
                ClientId::new(),
                None,
                Some("SUCCESS_FAST".to_string()),
            )
            .unwrap();
        job.state = JobState::Running;

        let event = lifecycle.transition(&mut job, JobState::Succeeded).unwrap();

        assert_eq!(job.state, JobState::Succeeded);
        assert_eq!(job.outcome, Some(JobOutcome::Success));
        assert_eq!(event.event_name, EventName::JobSucceeded);
    }

    #[test]
    fn given_valid_events_when_finalize_report_called_should_return_report() {
        let lifecycle = DummyLifecycle;
        let job_id = JobId::new();
        let t0 = Timestamp::now_utc();
        let t1 = Timestamp::from(t0.as_inner() + Duration::seconds(1));

        let events = vec![
            EventSnapshot {
                event_name: EventName::JobCreated,
                prev_state: JobState::Created,
                next_state: JobState::Created,
                timestamp: t0,
            },
            EventSnapshot {
                event_name: EventName::JobSucceeded,
                prev_state: JobState::Running,
                next_state: JobState::Succeeded,
                timestamp: t1,
            },
        ];

        let report = lifecycle
            .finalize_report(job_id, events, JobOutcome::Success, None)
            .unwrap();

        assert_eq!(report.job_id, job_id);
        assert_eq!(report.outcome, JobOutcome::Success);
    }

    #[test]
    fn given_missing_start_event_when_finalize_report_called_should_return_error() {
        let lifecycle = DummyLifecycle;
        let job_id = JobId::new();
        let t1 = Timestamp::now_utc();

        let events = vec![EventSnapshot {
            event_name: EventName::JobSucceeded,
            prev_state: JobState::Running,
            next_state: JobState::Succeeded,
            timestamp: t1,
        }];

        let result = lifecycle.finalize_report(job_id, events, JobOutcome::Success, None);

        assert_eq!(
            result,
            Err(crate::domain::entities::report::ReportError::MissingStartedEvent)
        );
    }
}
