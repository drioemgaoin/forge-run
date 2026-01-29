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
    ) -> Result<Job, JobValidationError> {
        // Step 1: Create the Job (entity validates invariants).
        let job = Job::new_instant(job_id, client_id, callback_url, work_kind)?;

        // Step 2: Persist the Job.
        // TODO(persistence): persist job.

        // Step 3: Emit and persist a JobCreated event.
        let created_event = Event::new_created(EventId::new(), job.id, Timestamp::now_utc());
        // TODO(persistence): persist created_event.

        // Step 4: Return the created job.
        Ok(job)
    }

    fn create_deferred(
        &self,
        job_id: JobId,
        client_id: ClientId,
        execution_at: Timestamp,
        callback_url: Option<String>,
        work_kind: Option<String>,
    ) -> Result<Job, JobValidationError> {
        // Step 1: Create the Job (entity validates invariants).
        let job = Job::new_deferred(job_id, client_id, execution_at, callback_url, work_kind)?;

        // Step 2: Persist the Job.
        // TODO(persistence): persist job.

        // Step 3: Emit and persist a JobCreated event.
        let created_event = Event::new_created(EventId::new(), job.id, Timestamp::now_utc());
        // TODO(persistence): persist created_event.

        // Step 4: Return the created job.
        Ok(job)
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
    #[test]
    fn given_valid_instant_job_when_created_should_return_job() {
        // TODO: implement
    }

    #[test]
    fn given_missing_work_kind_when_created_instant_should_return_error() {
        // TODO: implement
    }

    #[test]
    fn given_past_execution_at_when_created_deferred_should_return_error() {
        // TODO: implement
    }

    #[test]
    fn given_valid_transition_when_called_should_update_state_and_emit_event() {
        // TODO: implement
    }

    #[test]
    fn given_invalid_transition_when_called_should_return_error() {
        // TODO: implement
    }

    #[test]
    fn given_running_to_succeeded_when_transitioned_should_set_outcome() {
        // TODO: implement
    }

    #[test]
    fn given_valid_events_when_finalize_report_called_should_return_report() {
        // TODO: implement
    }

    #[test]
    fn given_missing_start_event_when_finalize_report_called_should_return_error() {
        // TODO: implement
    }
}
