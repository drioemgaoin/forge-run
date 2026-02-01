use crate::domain::entities::job::JobOutcome;

/// Deterministic mapping for each `work_kind` scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkPlan {
    pub duration_ms: u64,
    pub payload_kb: u64,
    pub outcome: JobOutcome,
    pub retryable: bool,
    pub outcome_reason: Option<String>,
}

/// Catalog of supported `work_kind` values and their deterministic behavior.
pub struct WorkCatalog;

impl WorkCatalog {
    pub const DEFAULT_MAX_RUNTIME_MS: u64 = 60_000;

    /// Resolve a work plan for the given kind and attempt.
    ///
    /// `attempt` is the number of retries already performed (0 for the first try).
    pub fn evaluate(work_kind: &str, attempt: u8, max_runtime_ms: u64) -> Option<WorkPlan> {
        // Step 1: Match the work kind to a deterministic plan.
        match work_kind {
            "SUCCESS_FAST" => Some(Self::success(1_000, 4)),
            "SUCCESS_NORMAL" => Some(Self::success(10_000, 16)),
            "SUCCESS_SLOW" => Some(Self::success(90_000, 32)),
            "FAIL_IMMEDIATE" => Some(Self::failed(500, 1, false, "fail_immediate")),
            "FAIL_AFTER_PROGRESS" => Some(Self::failed(20_000, 8, false, "fail_after_progress")),
            "FAIL_AFTER_RETRYABLE" => Some(Self::failed(5_000, 8, true, "retryable_failure")),
            "RUNS_LONG" => Some(Self::success(110_000, 32)),
            "RUNS_OVER_TIMEOUT" => Some(Self::failed(max_runtime_ms + 1_000, 8, false, "timeout")),
            "CPU_BURST" => Some(Self::success(8_000, 4)),
            "MEMORY_SPIKE" => Some(Self::success(12_000, 64)),
            "IO_HEAVY" => Some(Self::success(15_000, 32)),
            "MANY_SMALL_OUTPUTS" => Some(Self::success(9_000, 16)),
            "LARGE_OUTPUT" => Some(Self::success(9_000, 256)),
            "CANCEL_BEFORE_START" => Some(Self::canceled(5_000, 4)),
            "CANCEL_DURING_RUN" => Some(Self::canceled(10_000, 4)),
            "RETRY_ON_FAIL" => {
                if attempt == 0 {
                    Some(Self::failed(3_000, 4, true, "retry_on_fail"))
                } else {
                    Some(Self::success(3_000, 4))
                }
            }
            "RETRY_LIMIT_REACHED" => Some(Self::failed(3_000, 4, false, "retry_limit")),
            "DUPLICATE_SUBMIT_SAME_KEY" => Some(Self::success(2_000, 4)),
            "DUPLICATE_SUBMIT_DIFFERENT_KEY" => Some(Self::success(2_000, 4)),
            "WEBHOOK_SUCCESS" => Some(Self::success(2_000, 4)),
            "WEBHOOK_TIMEOUT" => Some(Self::success(2_000, 4)),
            "WEBHOOK_5XX" => Some(Self::success(2_000, 4)),
            "WEBHOOK_RETRIES_EXHAUSTED" => Some(Self::success(2_000, 4)),
            "WEBHOOK_SLOW_RECEIVER" => Some(Self::success(2_000, 4)),
            "SCHEDULED_ON_TIME" => Some(Self::success(2_000, 4)),
            "SCHEDULED_LATE_RECOVERY" => Some(Self::success(2_000, 4)),
            "SCHEDULED_FAR_FUTURE" => Some(Self::success(2_000, 4)),
            "PAYLOAD_SMALL" => Some(Self::success(2_000, 1)),
            "PAYLOAD_MEDIUM" => Some(Self::success(2_000, 16)),
            "PAYLOAD_LARGE" => Some(Self::success(2_000, 256)),
            "PAYLOAD_INVALID" => Some(Self::failed(0, 0, false, "payload_invalid")),
            _ => None,
        }
    }

    fn success(duration_ms: u64, payload_kb: u64) -> WorkPlan {
        // Step 1: Build a successful plan with a deterministic payload size.
        WorkPlan {
            duration_ms,
            payload_kb,
            outcome: JobOutcome::Success,
            retryable: false,
            outcome_reason: None,
        }
    }

    fn canceled(duration_ms: u64, payload_kb: u64) -> WorkPlan {
        // Step 1: Build a canceled plan with a standard reason.
        WorkPlan {
            duration_ms,
            payload_kb,
            outcome: JobOutcome::Canceled,
            retryable: false,
            outcome_reason: Some("canceled".to_string()),
        }
    }

    fn failed(duration_ms: u64, payload_kb: u64, retryable: bool, reason: &str) -> WorkPlan {
        // Step 1: Build a failed plan with a deterministic reason.
        WorkPlan {
            duration_ms,
            payload_kb,
            outcome: JobOutcome::Failed,
            retryable,
            outcome_reason: Some(reason.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkCatalog;
    use crate::domain::entities::job::JobOutcome;

    #[test]
    fn given_success_work_kind_when_evaluated_should_return_success_plan() {
        let plan = WorkCatalog::evaluate("SUCCESS_FAST", 0, WorkCatalog::DEFAULT_MAX_RUNTIME_MS)
            .expect("plan should exist");

        assert_eq!(plan.outcome, JobOutcome::Success);
        assert_eq!(plan.duration_ms, 1_000);
    }

    #[test]
    fn given_retry_on_fail_when_first_attempt_should_return_retryable_failure() {
        let plan = WorkCatalog::evaluate("RETRY_ON_FAIL", 0, WorkCatalog::DEFAULT_MAX_RUNTIME_MS)
            .expect("plan should exist");

        assert_eq!(plan.outcome, JobOutcome::Failed);
        assert!(plan.retryable);
    }

    #[test]
    fn given_retry_on_fail_when_second_attempt_should_return_success() {
        let plan = WorkCatalog::evaluate("RETRY_ON_FAIL", 1, WorkCatalog::DEFAULT_MAX_RUNTIME_MS)
            .expect("plan should exist");

        assert_eq!(plan.outcome, JobOutcome::Success);
    }
}
