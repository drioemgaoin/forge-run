// Use case: worker_loop.

use crate::application::context::AppContext;
use crate::application::usecases::run_worker_once::{
    RunWorkerOnceError, RunWorkerOnceUseCase, WorkerConfig,
};
use time::Duration;

/// Runs the worker loop continuously until a shutdown signal is received.
pub struct WorkerLoopUseCase;

#[derive(Debug)]
pub enum WorkerLoopError {
    Worker(RunWorkerOnceError),
}

impl WorkerLoopUseCase {
    /// Run the worker loop, polling the queue at a fixed interval.
    pub async fn run(
        ctx: &AppContext,
        worker_id: &str,
        config: &WorkerConfig,
        poll_interval: Duration,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), WorkerLoopError> {
        // Step 1: Loop until the shutdown signal is triggered.
        loop {
            if *shutdown.borrow() {
                break;
            }

            // Step 2: Attempt to process a single job.
            let _ = RunWorkerOnceUseCase::execute(ctx, worker_id, config)
                .await
                .map_err(WorkerLoopError::Worker)?;

            // Step 3: Sleep until the next poll or shutdown.
            let sleep_duration =
                std::time::Duration::from_millis(poll_interval.whole_milliseconds().max(0) as u64);

            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(sleep_duration) => {}
            }
        }

        // Step 4: Exit cleanly once shutdown is signaled.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerLoopUseCase;
    use crate::application::context::test_support::test_context;
    use crate::application::usecases::run_worker_once::WorkerConfig;
    use time::Duration;

    #[tokio::test]
    async fn given_shutdown_signal_when_run_should_exit_cleanly() {
        let ctx = test_context();
        let config = WorkerConfig::default();
        let (_tx, rx) = tokio::sync::watch::channel(true);

        let result =
            WorkerLoopUseCase::run(&ctx, "worker-1", &config, Duration::milliseconds(100), rx)
                .await;

        assert!(result.is_ok());
    }
}
