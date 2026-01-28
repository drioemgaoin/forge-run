// Use case: submit_job.
// Orchestrates validation, job definition generation, persistence, and enqueue.
// No implementation yet.

use crate::domain::services::job_definition::JobDefinitionGenerator;

#[allow(dead_code)]
pub struct SubmitJobUseCase<G: JobDefinitionGenerator> {
    pub generator: G,
}
