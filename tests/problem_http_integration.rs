use axum::body::Body;
use axum::body::to_bytes;
use axum::http::{HeaderValue, Request, StatusCode};
use forge_run::application::context::AppContext;
use forge_run::config::Settings;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn setup_state() -> Option<AppState> {
    let url = test_db_url()?;
    let db = Arc::new(PostgresDatabase::connect(&url).await.ok()?);
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
    let ctx = AppContext::new(repos, Arc::new(lifecycle), settings.clone());
    let state = AppState { ctx: Arc::new(ctx) };
    Some(state)
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
}

fn is_problem_json(content_type: Option<&HeaderValue>) -> bool {
    content_type
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/problem+json"))
        .unwrap_or(false)
}

#[tokio::test]
async fn given_invalid_client_id_when_create_key_should_return_problem_details() {
    let Some(state) = setup_state().await else {
        return;
    };

    let response = http::app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/clients/not-a-uuid/keys")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"rotate":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(is_problem_json(response.headers().get("content-type")));
    assert!(response.headers().get("x-request-id").is_some());
    let json = response_json(response).await;
    assert_eq!(
        json.get("code"),
        Some(&Value::String("RFA_REQUEST_MALFORMED".to_string()))
    );
    assert!(json.get("trace_id").is_some());
}

#[tokio::test]
async fn given_invalid_job_type_when_submit_should_return_problem_details() {
    let Some(state) = setup_state().await else {
        return;
    };

    let response = http::app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"type":"BOGUS","work_kind":"SUCCESS_FAST"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(is_problem_json(response.headers().get("content-type")));
    assert!(response.headers().get("x-request-id").is_some());
    let json = response_json(response).await;
    assert_eq!(
        json.get("code"),
        Some(&Value::String("RFA_REQUEST_MALFORMED".to_string()))
    );
    assert!(json.get("trace_id").is_some());
}
