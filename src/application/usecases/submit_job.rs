// Use case: submit_job.
// Orchestrates validation, persistence, and enqueue.
// No implementation yet.

use crate::domain::services::job_lifecycle::JobLifecycleService;

#[allow(dead_code)]
pub struct SubmitJobUseCase<S: JobLifecycleService> {
    pub lifecycle: S,
}
