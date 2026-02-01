// Use case: worker_manager.

use crate::application::context::AppContext;
use crate::application::usecases::run_worker_once::WorkerConfig;
use crate::application::usecases::worker_loop::WorkerLoopUseCase;
use crate::config::Workers;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use time::Duration;
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum WorkerManagerError {
    Storage(String),
}

struct WorkerHandle {
    #[allow(dead_code)]
    id: String,
    shutdown: tokio::sync::watch::Sender<bool>,
    #[allow(dead_code)]
    join: tokio::task::JoinHandle<()>,
}

/// Manages worker tasks and auto-scaling based on queue depth.
pub struct WorkerManager {
    ctx: Arc<AppContext>,
    config: WorkerConfig,
    poll_interval: Duration,
    min_workers: usize,
    max_workers: usize,
    scale_interval: Duration,
    handles: Mutex<Vec<WorkerHandle>>,
    next_id: AtomicUsize,
}

impl WorkerManager {
    /// Build a manager from application settings.
    pub fn new(ctx: Arc<AppContext>, settings: &Workers) -> Self {
        // Step 1: Build worker execution config from settings.
        let config = WorkerConfig {
            lease_timeout: Duration::seconds(settings.lease_timeout_seconds as i64),
            retry_policy: crate::domain::workflows::retry_policy::RetryPolicy::default(),
            max_runtime_ms:
                crate::domain::workflows::work_catalog::WorkCatalog::DEFAULT_MAX_RUNTIME_MS,
        };

        // Step 2: Store worker management settings.
        Self {
            ctx,
            config,
            poll_interval: Duration::milliseconds(settings.poll_interval_ms as i64),
            min_workers: settings.default_count.max(1),
            max_workers: settings.max_count.max(1),
            scale_interval: Duration::milliseconds(settings.scale_interval_ms as i64),
            handles: Mutex::new(Vec::new()),
            next_id: AtomicUsize::new(1),
        }
    }

    /// Start workers and the scaling loop.
    pub async fn start(self: Arc<Self>) -> Result<(), WorkerManagerError> {
        // Step 1: Spawn the default number of workers.
        self.set_target_count(self.min_workers).await?;

        // Step 2: Launch the scaling loop in the background.
        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_scale_loop().await;
        });

        Ok(())
    }

    async fn run_scale_loop(self: Arc<Self>) {
        // Step 1: Scale continuously at a fixed interval.
        loop {
            let _ = self.scale_once().await;
            tokio::time::sleep(std::time::Duration::from_millis(
                self.scale_interval.whole_milliseconds().max(0) as u64,
            ))
            .await;
        }
    }

    async fn scale_once(&self) -> Result<(), WorkerManagerError> {
        // Step 1: Read current queue depth from the repository.
        let queue_depth = self
            .ctx
            .repos
            .job
            .queue_depth()
            .await
            .map_err(|e| WorkerManagerError::Storage(format!("{e:?}")))?;

        // Step 2: Compute desired worker count.
        let desired = (queue_depth as usize)
            .max(self.min_workers)
            .min(self.max_workers);

        // Step 3: Scale to the desired count.
        self.set_target_count(desired).await
    }

    async fn set_target_count(&self, target: usize) -> Result<(), WorkerManagerError> {
        // Step 1: Acquire the handle list for modification.
        let mut handles = self.handles.lock().await;

        // Step 2: Scale up to the target.
        while handles.len() < target {
            handles.push(self.spawn_worker());
        }

        // Step 3: Scale down to the target.
        while handles.len() > target {
            if let Some(handle) = handles.pop() {
                let _ = handle.shutdown.send(true);
            }
        }

        Ok(())
    }

    fn spawn_worker(&self) -> WorkerHandle {
        // Step 1: Allocate a unique worker id.
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let worker_id = format!("worker-{id}");
        let worker_label = worker_id.clone();

        // Step 2: Build a shutdown channel for this worker.
        let (tx, rx) = tokio::sync::watch::channel(false);

        // Step 3: Spawn the worker loop task.
        let ctx = self.ctx.clone();
        let config = self.config.clone();
        let poll_interval = self.poll_interval;
        let handle = tokio::spawn(async move {
            let _ = WorkerLoopUseCase::run(&ctx, &worker_label, &config, poll_interval, rx).await;
        });

        WorkerHandle {
            id: worker_id,
            shutdown: tx,
            join: handle,
        }
    }
}

#[cfg(test)]
impl WorkerManager {
    async fn test_scale_once(&self) -> Result<(), WorkerManagerError> {
        self.scale_once().await
    }

    async fn test_worker_count(&self) -> usize {
        self.handles.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerManager;
    use crate::application::context::test_support::test_context;
    use crate::config::Workers;
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::repositories::job_repository::JobRepository;
    use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    #[derive(Clone)]
    struct DummyStore {
        depth: Arc<Mutex<u64>>,
    }

    #[async_trait]
    impl JobStore for DummyStore {
        async fn get(&self, _job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn insert(&self, _row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn update(&self, _row: &JobRow) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn delete(&self, _job_id: uuid::Uuid) -> Result<(), JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn list_due_deferred(
            &self,
            _now: OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn claim_next_queued(
            &self,
            _worker_id: &str,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn list_expired_leases(
            &self,
            _now: OffsetDateTime,
            _limit: u32,
        ) -> Result<Vec<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn heartbeat(
            &self,
            _job_id: uuid::Uuid,
            _worker_id: &str,
            _heartbeat_at: OffsetDateTime,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn queue_depth(&self) -> Result<u64, JobRepositoryError> {
            Ok(*self.depth.lock().unwrap())
        }

        async fn count_scheduled_at(
            &self,
            _scheduled_at: OffsetDateTime,
            _tolerance_ms: u64,
        ) -> Result<u64, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn next_due_time(
            &self,
            _now: OffsetDateTime,
        ) -> Result<Option<OffsetDateTime>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &JobRow,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &JobRow,
        ) -> Result<JobRow, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<(), JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }

        async fn claim_next_queued_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _worker_id: &str,
            _lease_expires_at: OffsetDateTime,
        ) -> Result<Option<JobRow>, JobRepositoryError> {
            Err(JobRepositoryError::InvalidInput)
        }
    }

    fn settings(default_count: usize, max_count: usize) -> Workers {
        Workers {
            default_count,
            max_count,
            poll_interval_ms: 250,
            lease_timeout_seconds: 30,
            scale_interval_ms: 1000,
        }
    }

    #[tokio::test]
    async fn given_queue_depth_when_scale_once_should_respect_max() {
        let mut ctx = test_context();
        let depth = Arc::new(Mutex::new(10_u64));
        let store = DummyStore {
            depth: depth.clone(),
        };
        ctx.repos.job = Arc::new(JobRepository::new(Arc::new(store)));
        let ctx = Arc::new(ctx);

        let manager = Arc::new(WorkerManager::new(ctx, &settings(1, 3)));
        manager.test_scale_once().await.unwrap();

        assert_eq!(manager.test_worker_count().await, 3);
    }

    #[tokio::test]
    async fn given_low_queue_depth_when_scale_once_should_keep_min() {
        let mut ctx = test_context();
        let depth = Arc::new(Mutex::new(0_u64));
        let store = DummyStore {
            depth: depth.clone(),
        };
        ctx.repos.job = Arc::new(JobRepository::new(Arc::new(store)));
        let ctx = Arc::new(ctx);

        let manager = Arc::new(WorkerManager::new(ctx, &settings(2, 5)));
        manager.test_scale_once().await.unwrap();

        assert_eq!(manager.test_worker_count().await, 2);
    }
}
