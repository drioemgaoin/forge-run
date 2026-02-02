use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use forge_run::application::context::AppContext;
use forge_run::application::usecases::deliver_webhooks::DeliverWebhooksUseCase;
use forge_run::config::Settings;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::domain::value_objects::ids::{ClientId, EventId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::dto::WebhookRow;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use serde_json::Value;
use std::sync::{Arc, Mutex};

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn base_settings(db_url: String) -> Settings {
    Settings {
        server: forge_run::config::Server {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        db: forge_run::config::Db { url: db_url },
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
        observability: forge_run::config::Observability {
            service_name: "forge-run".to_string(),
            enable_tracing: false,
            otlp_endpoint: "http://127.0.0.1:4317".to_string(),
            enable_metrics: false,

            log_file_path: None,
        },
    }
}

async fn spawn_webhook_server() -> (String, Arc<Mutex<Vec<Value>>>) {
    let received: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let state = received.clone();
    let app = Router::new()
        .route(
            "/hook",
            post(
                |State(state): State<Arc<Mutex<Vec<Value>>>>, body: Bytes| async move {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                    state.lock().unwrap().push(parsed);
                    StatusCode::OK
                },
            ),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind webhook server");
    let addr = listener.local_addr().expect("get addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (format!("http://{}/hook", addr), received)
}

#[tokio::test]
async fn given_due_delivery_when_run_once_should_mark_delivered() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let settings = base_settings(url);
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle), settings);

    let (webhook_url, received) = spawn_webhook_server().await;
    let client_id = ClientId::new();
    ctx.repos
        .client
        .insert(&forge_run::domain::entities::client::Client::new(client_id))
        .await
        .unwrap();

    let webhook = WebhookRow {
        id: uuid::Uuid::new_v4(),
        client_id: client_id.0,
        url: webhook_url,
        events: vec!["job_created".to_string()],
        is_default: true,
        created_at: Timestamp::now_utc().as_inner(),
    };
    ctx.repos.webhook.insert(&webhook).await.unwrap();

    let (job, _) = ctx
        .job_lifecycle
        .create_instant(
            JobId::new(),
            client_id,
            None,
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .await
        .unwrap();

    let due = ctx
        .repos
        .webhook_delivery
        .list_due(Timestamp::now_utc().as_inner(), 10)
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    let delivery_id = due[0].id;

    let result = DeliverWebhooksUseCase::run_once(&ctx, Timestamp::now_utc(), 10)
        .await
        .unwrap();
    assert_eq!(result.delivered, 1);

    let updated = ctx
        .repos
        .webhook_delivery
        .get(delivery_id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.is_delivered());

    let received_count = received.lock().unwrap().len();
    assert_eq!(received_count, 1);

    ctx.repos
        .webhook_delivery
        .delete(delivery_id)
        .await
        .unwrap();
    let events = ctx.repos.event.list_by_job_id(job.id).await.unwrap();
    for event in events {
        ctx.repos.event.delete(EventId(event.id)).await.unwrap();
    }
    ctx.repos.job.delete(job.id).await.unwrap();
    ctx.repos.webhook.delete(webhook.id).await.unwrap();
    ctx.repos.client.delete(client_id).await.unwrap();
}

#[tokio::test]
async fn given_failed_delivery_when_max_attempts_reached_should_mark_failed() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let mut settings = base_settings(url);
    settings.webhook_delivery.max_attempts = 1;
    settings.webhook_delivery.request_timeout_ms = 200;
    let ctx = AppContext::new(repos.clone(), Arc::new(lifecycle), settings);

    let client_id = ClientId::new();
    ctx.repos
        .client
        .insert(&forge_run::domain::entities::client::Client::new(client_id))
        .await
        .unwrap();
    let webhook = WebhookRow {
        id: uuid::Uuid::new_v4(),
        client_id: client_id.0,
        url: "http://127.0.0.1:9/hook".to_string(),
        events: vec!["job_created".to_string()],
        is_default: true,
        created_at: Timestamp::now_utc().as_inner(),
    };
    ctx.repos.webhook.insert(&webhook).await.unwrap();

    let (job, _) = ctx
        .job_lifecycle
        .create_instant(
            JobId::new(),
            client_id,
            None,
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .await
        .unwrap();

    let due = ctx
        .repos
        .webhook_delivery
        .list_due(Timestamp::now_utc().as_inner(), 10)
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    let delivery_id = due[0].id;

    let result = DeliverWebhooksUseCase::run_once(&ctx, Timestamp::now_utc(), 10)
        .await
        .unwrap();
    assert_eq!(result.failed, 1);

    let updated = ctx
        .repos
        .webhook_delivery
        .get(delivery_id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated.is_failed());

    ctx.repos
        .webhook_delivery
        .delete(delivery_id)
        .await
        .unwrap();
    let events = ctx.repos.event.list_by_job_id(job.id).await.unwrap();
    for event in events {
        ctx.repos.event.delete(EventId(event.id)).await.unwrap();
    }
    ctx.repos.job.delete(job.id).await.unwrap();
    ctx.repos.webhook.delete(webhook.id).await.unwrap();
    ctx.repos.client.delete(client_id).await.unwrap();
}
