use crate::domain::entities::job::JobState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    Forbidden,
}

pub struct JobStateMachine;

impl JobStateMachine {
    pub fn can_transition(from: JobState, to: JobState) -> bool {
        matches!(
            (from, to),
            (JobState::Created, JobState::Queued)
                | (JobState::Created, JobState::Canceled)
                | (JobState::Queued, JobState::Assigned)
                | (JobState::Queued, JobState::Canceled)
                | (JobState::Assigned, JobState::Running)
                | (JobState::Assigned, JobState::Canceled)
                | (JobState::Running, JobState::Succeeded)
                | (JobState::Running, JobState::Failed)
                | (JobState::Running, JobState::Canceled)
                | (JobState::Failed, JobState::Queued)
        )
    }

    pub fn transition(from: JobState, to: JobState) -> Result<JobState, TransitionError> {
        if Self::can_transition(from, to) {
            return Ok(to);
        }

        Err(TransitionError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_allowed_transitions_when_checked_should_be_allowed() {
        let cases = [
            (JobState::Created, JobState::Queued),
            (JobState::Created, JobState::Canceled),
            (JobState::Queued, JobState::Assigned),
            (JobState::Queued, JobState::Canceled),
            (JobState::Assigned, JobState::Running),
            (JobState::Assigned, JobState::Canceled),
            (JobState::Running, JobState::Succeeded),
            (JobState::Running, JobState::Failed),
            (JobState::Running, JobState::Canceled),
            (JobState::Failed, JobState::Queued),
        ];

        for (from, to) in cases {
            assert!(JobStateMachine::can_transition(from, to));
            assert_eq!(JobStateMachine::transition(from, to), Ok(to));
        }
    }

    #[test]
    fn given_final_states_when_transitioning_should_be_forbidden() {
        let states = [
            JobState::Created,
            JobState::Queued,
            JobState::Assigned,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Canceled,
        ];

        for to in states {
            assert_eq!(JobStateMachine::transition(JobState::Succeeded, to), Err(TransitionError::Forbidden));
            assert_eq!(JobStateMachine::transition(JobState::Canceled, to), Err(TransitionError::Forbidden));
        }
    }

    #[test]
    fn given_all_state_pairs_when_checked_should_match_allowed_matrix() {
        let states = [
            JobState::Created,
            JobState::Queued,
            JobState::Assigned,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Canceled,
        ];

        for from in states {
            for to in states {
                let allowed = JobStateMachine::can_transition(from, to);
                let result = JobStateMachine::transition(from, to);
                if allowed {
                    assert_eq!(result, Ok(to));
                } else {
                    assert_eq!(result, Err(TransitionError::Forbidden));
                }
            }
        }
    }
}



