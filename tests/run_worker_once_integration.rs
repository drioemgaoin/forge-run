use forge_run::application::context::AppContext;
use forge_run::application::usecases::run_worker_once::{RunWorkerOnceUseCase, WorkerConfig};
use forge_run::application::usecases::submit_job::{SubmitJobCommand, SubmitJobUseCase};
use forge_run::config::Settings;
use forge_run::domain::entities::client::Client;
use forge_run::domain::entities::job::JobState;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::domain::value_objects::ids::{ClientId, EventId};
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use std::sync::Arc;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn given_queued_job_when_run_worker_once_should_finish_and_report() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let settings = Settings {
        server: forge_run::config::Server {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        db: forge_run::config::Db { url },
        redis: forge_run::config::Redis {
            url: "redis://127.0.0.1/".to_string(),
        },
        workers: forge_run::config::Workers {
            default_count: 1,
            max_count: 1,
            poll_interval_ms: 250,
            lease_timeout_seconds: 30,
            scale_interval_ms: 1000,
        },
        scheduler: forge_run::config::Scheduler {
            poll_interval_ms: 1000,
            max_batch: 100,
            skew_seconds: 1,
            tolerance_ms: 100,
        },
        webhook_delivery: forge_run::config::WebhookDelivery {
            poll_interval_ms: 1000,
            batch_size: 100,
            request_timeout_ms: 2000,
            max_attempts: 5,
            backoff_initial_ms: 500,
            backoff_max_ms: 30000,
        },
    };
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle), settings.clone());

    let client_id = ClientId::new();
    repos.client.insert(&Client::new(client_id)).await.unwrap();

    let submit = SubmitJobUseCase::execute(
        &ctx,
        SubmitJobCommand {
            client_id,
            execution_at: None,
            callback_url: None,
            callback_events: None,
            work_kind: Some("SUCCESS_FAST".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(submit.job.state, JobState::Queued);

    let config = WorkerConfig::default();
    let result = RunWorkerOnceUseCase::execute(&ctx, "worker-1", &config)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(result.job.state, JobState::Succeeded);
    assert!(result.report.is_some());

    let events = repos.event.list_by_job_id(result.job.id).await.unwrap();
    for event in events {
        repos.event.delete(EventId(event.id)).await.unwrap();
    }
    repos.report.delete(result.job.id).await.unwrap();
    repos.job.delete(result.job.id).await.unwrap();
}

#[tokio::test]
async fn given_retryable_job_when_run_worker_once_should_requeue_with_backoff() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let settings = Settings {
        server: forge_run::config::Server {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        db: forge_run::config::Db { url },
        redis: forge_run::config::Redis {
            url: "redis://127.0.0.1/".to_string(),
        },
        workers: forge_run::config::Workers {
            default_count: 1,
            max_count: 1,
            poll_interval_ms: 250,
            lease_timeout_seconds: 30,
            scale_interval_ms: 1000,
        },
        scheduler: forge_run::config::Scheduler {
            poll_interval_ms: 1000,
            max_batch: 100,
            skew_seconds: 1,
            tolerance_ms: 100,
        },
        webhook_delivery: forge_run::config::WebhookDelivery {
            poll_interval_ms: 1000,
            batch_size: 100,
            request_timeout_ms: 2000,
            max_attempts: 5,
            backoff_initial_ms: 500,
            backoff_max_ms: 30000,
        },
    };
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle), settings.clone());

    let client_id = ClientId::new();
    repos.client.insert(&Client::new(client_id)).await.unwrap();

    let _submit = SubmitJobUseCase::execute(
        &ctx,
        SubmitJobCommand {
            client_id,
            execution_at: None,
            callback_url: None,
            callback_events: None,
            work_kind: Some("RETRY_ON_FAIL".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    let config = WorkerConfig::default();
    let result = RunWorkerOnceUseCase::execute(&ctx, "worker-1", &config)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(result.job.state, JobState::Queued);
    assert!(result.job.executed_at.is_some());

    let events = repos.event.list_by_job_id(result.job.id).await.unwrap();
    for event in events {
        repos.event.delete(EventId(event.id)).await.unwrap();
    }
    repos.job.delete(result.job.id).await.unwrap();
}
