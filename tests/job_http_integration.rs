use axum::body::Body;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use forge_run::application::context::AppContext;
use forge_run::config::Settings;
use forge_run::domain::entities::event::{Event, EventName};
use forge_run::domain::entities::job::{Job, JobOutcome, JobState};
use forge_run::domain::entities::report::{EventSnapshot, Report};
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::domain::value_objects::ids::{ClientId, EventId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use serde_json::Value;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tower::util::ServiceExt;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn setup_state() -> Option<(AppState, Arc<PostgresDatabase>)> {
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
        observability: forge_run::config::Observability {
            service_name: "forge-run".to_string(),
            enable_tracing: false,
            otlp_endpoint: "http://127.0.0.1:4317".to_string(),
            enable_metrics: false,

            log_file_path: None,
        },
    };
    let ctx = AppContext::new(repos, Arc::new(lifecycle), settings.clone());
    let state = AppState {
        ctx: Arc::new(ctx),
        metrics: None,
    };
    Some((state, db))
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
}

fn bearer_request(method: &str, uri: String, api_key: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(body)
        .unwrap()
}

async fn create_client_id(state: AppState) -> uuid::Uuid {
    let response = http::app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/clients")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = response_json(response).await;
    let id = json
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    uuid::Uuid::parse_str(id).unwrap()
}

async fn create_api_key(state: AppState, client_id: uuid::Uuid) -> String {
    let response = http::app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/clients/{client_id}/keys"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"rotate":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let json = response_json(response).await;
    json.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn given_valid_request_when_submit_and_get_then_return_job() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;

    let submit = http::app(state.clone())
        .oneshot(bearer_request(
            "POST",
            "/jobs".to_string(),
            &api_key,
            Body::from(r#"{"type":"EXECUTE","work_kind":"SUCCESS_FAST"}"#),
        ))
        .await
        .unwrap();

    assert_eq!(submit.status(), StatusCode::ACCEPTED);
    let json = response_json(submit).await;
    let job_id = json.get("job_id").and_then(|v| v.as_str()).unwrap();

    let get = http::app(state.clone())
        .oneshot(bearer_request(
            "GET",
            format!("/jobs/{job_id}"),
            &api_key,
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);

    let cancel = http::app(state.clone())
        .oneshot(bearer_request(
            "POST",
            format!("/jobs/{job_id}/cancel"),
            &api_key,
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(cancel.status(), StatusCode::OK);

    let job_id = JobId(uuid::Uuid::parse_str(job_id).unwrap());
    let events = state.ctx.repos.event.list_by_job_id(job_id).await.unwrap();
    for event in events {
        state
            .ctx
            .repos
            .event
            .delete(EventId(event.id))
            .await
            .unwrap();
    }
    state.ctx.repos.job.delete(job_id).await.unwrap();
}

#[tokio::test]
async fn given_idempotency_key_when_submitting_twice_should_return_same_job() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;
    let key = "same-key-123";

    let submit = || {
        http::app(state.clone()).oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {api_key}"))
                .header("Idempotency-Key", key)
                .body(Body::from(
                    r#"{"type":"EXECUTE","work_kind":"SUCCESS_FAST"}"#,
                ))
                .unwrap(),
        )
    };

    let first = submit().await.unwrap();
    let second = submit().await.unwrap();

    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(second.status(), StatusCode::ACCEPTED);

    let first_json = response_json(first).await;
    let second_json = response_json(second).await;
    let first_id = first_json.get("job_id").and_then(|v| v.as_str()).unwrap();
    let second_id = second_json.get("job_id").and_then(|v| v.as_str()).unwrap();
    assert_eq!(first_id, second_id);

    let job_id = JobId(uuid::Uuid::parse_str(first_id).unwrap());
    let events = state.ctx.repos.event.list_by_job_id(job_id).await.unwrap();
    for event in events {
        state
            .ctx
            .repos
            .event
            .delete(EventId(event.id))
            .await
            .unwrap();
    }
    state.ctx.repos.job.delete(job_id).await.unwrap();
    state
        .ctx
        .repos
        .idempotency
        .delete(client_id, key)
        .await
        .unwrap();
}

#[tokio::test]
async fn given_report_exists_when_get_report_should_return_report() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;

    let job = Job::new_instant(
        JobId::new(),
        ClientId(client_id),
        None,
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    state.ctx.repos.job.insert(&job).await.unwrap();

    let t0 = Timestamp::now_utc();
    let t1 = Timestamp::from(OffsetDateTime::now_utc() + Duration::seconds(1));
    let created = Event::new_created(EventId::new(), job.id, t0);
    let succeeded = Event::from_transition(
        EventId::new(),
        job.id,
        JobState::Running,
        JobState::Succeeded,
    )
    .unwrap();
    state.ctx.repos.event.insert(&created).await.unwrap();
    state.ctx.repos.event.insert(&succeeded).await.unwrap();

    let events = vec![
        EventSnapshot {
            event_name: EventName::JobCreated,
            prev_state: JobState::Created,
            next_state: JobState::Created,
            timestamp: t0,
        },
        EventSnapshot {
            event_name: EventName::JobSucceeded,
            prev_state: JobState::Running,
            next_state: JobState::Succeeded,
            timestamp: t1,
        },
    ];
    let report = Report::from_events(job.id, JobOutcome::Success, None, events).unwrap();
    state.ctx.repos.report.insert(&report).await.unwrap();

    let get = http::app(state.clone())
        .oneshot(bearer_request(
            "GET",
            format!("/jobs/{}/report", job.id.0),
            &api_key,
            Body::empty(),
        ))
        .await
        .unwrap();

    assert_eq!(get.status(), StatusCode::OK);

    state.ctx.repos.report.delete(job.id).await.unwrap();
    let event_rows = state.ctx.repos.event.list_by_job_id(job.id).await.unwrap();
    for row in event_rows {
        state.ctx.repos.event.delete(EventId(row.id)).await.unwrap();
    }
    state.ctx.repos.job.delete(job.id).await.unwrap();
}
