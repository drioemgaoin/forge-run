// Use case: heartbeat_job.

use crate::application::context::AppContext;
use crate::domain::entities::job::Job;
use crate::domain::value_objects::ids::JobId;
use time::Duration;

/// Records a heartbeat for a leased job and extends its lease.
pub struct HeartbeatJobUseCase;

#[derive(Debug)]
pub enum HeartbeatJobError {
    NotFound,
    Storage(String),
}

impl HeartbeatJobUseCase {
    /// Record a heartbeat and extend the lease for a job owned by a worker.
    pub async fn execute(
        ctx: &AppContext,
        job_id: JobId,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Job, HeartbeatJobError> {
        // Step 1: Ask the repository to record the heartbeat.
        let updated = ctx
            .repos
            .job
            .heartbeat(job_id, worker_id, lease_duration)
            .await
            .map_err(|e| match e {
                crate::infrastructure::db::stores::job_store::JobRepositoryError::NotFound => {
                    HeartbeatJobError::NotFound
                }
                _ => HeartbeatJobError::Storage(format!("{e:?}")),
            })?;

        // Step 2: Return the updated job view.
        Ok(updated)
    }
}
