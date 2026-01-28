use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    Instant,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Created,
    Queued,
    Assigned,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobOutcome {
    Success,
    Failed,
    Canceled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_state_has_expected_variants() {
        let states = [
            JobState::Created,
            JobState::Queued,
            JobState::Assigned,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
            JobState::Canceled,
        ];

        assert_eq!(states.len(), 7);
    }
}
