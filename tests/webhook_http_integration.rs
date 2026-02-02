use axum::body::Body;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use forge_run::application::context::AppContext;
use forge_run::config::Settings;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::infrastructure::db::dto::WebhookRow;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use forge_run::infrastructure::db::repositories::webhook_repository::WebhookRepository;
use forge_run::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

struct DummyWebhookStore {
    rows: Mutex<HashMap<uuid::Uuid, WebhookRow>>,
    urls: Mutex<HashMap<String, uuid::Uuid>>,
}

impl DummyWebhookStore {
    fn new() -> Self {
        Self {
            rows: Mutex::new(HashMap::new()),
            urls: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl WebhookStore for DummyWebhookStore {
    async fn get(
        &self,
        webhook_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        Ok(self.rows.lock().unwrap().get(&webhook_id).cloned())
    }

    async fn get_default_for_client(
        &self,
        _client_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        let rows = self.rows.lock().unwrap();
        Ok(rows.values().find(|row| row.is_default).cloned())
    }

    async fn insert(&self, row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError> {
        let mut urls = self.urls.lock().unwrap();
        if urls.contains_key(&row.url) {
            return Err(WebhookRepositoryError::Conflict);
        }
        if row.is_default {
            let rows = self.rows.lock().unwrap();
            if rows
                .values()
                .any(|stored| stored.client_id == row.client_id && stored.is_default)
            {
                return Err(WebhookRepositoryError::Conflict);
            }
        }
        urls.insert(row.url.clone(), row.id);
        self.rows.lock().unwrap().insert(row.id, row.clone());
        Ok(row.clone())
    }

    async fn delete(&self, webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
        let mut rows = self.rows.lock().unwrap();
        let Some(row) = rows.remove(&webhook_id) else {
            return Err(WebhookRepositoryError::NotFound);
        };
        self.urls.lock().unwrap().remove(&row.url);
        Ok(())
    }
}

async fn setup_state() -> Option<(AppState, Arc<PostgresDatabase>)> {
    let url = test_db_url()?;
    let db = Arc::new(PostgresDatabase::connect(&url).await.ok()?);
    let mut repos = Repositories::postgres(db.clone());
    let webhook_repo = WebhookRepository::new(Arc::new(DummyWebhookStore::new()));
    repos.webhook = Arc::new(webhook_repo);
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
async fn given_valid_request_when_register_webhook_should_return_created() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;

    let register = http::app(state)
        .oneshot(bearer_request(
            "POST",
            format!("/clients/{client_id}/webhooks"),
            &api_key,
            Body::from(
                r#"{"url":"https://example.com/hook","events":["job_created"],"is_default":true}"#,
            ),
        ))
        .await
        .unwrap();

    assert_eq!(register.status(), StatusCode::CREATED);
    let json = response_json(register).await;
    assert!(json.get("webhook_id").is_some());
    assert!(json.get("created_at").is_some());
}

#[tokio::test]
async fn given_duplicate_webhook_when_register_should_return_conflict() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;

    let first = http::app(state.clone())
        .oneshot(bearer_request(
            "POST",
            format!("/clients/{client_id}/webhooks"),
            &api_key,
            Body::from(
                r#"{"url":"https://example.com/hook","events":["job_created"],"is_default":true}"#,
            ),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = http::app(state)
        .oneshot(bearer_request(
            "POST",
            format!("/clients/{client_id}/webhooks"),
            &api_key,
            Body::from(
                r#"{"url":"https://example.com/hook","events":["job_created"],"is_default":true}"#,
            ),
        ))
        .await
        .unwrap();

    assert_eq!(second.status(), StatusCode::CONFLICT);
    let json = response_json(second).await;
    assert_eq!(
        json.get("code"),
        Some(&Value::String("RFA_JOB_CONFLICT".to_string()))
    );
}

#[tokio::test]
async fn given_unknown_webhook_when_unregister_should_return_not_found() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;

    let response = http::app(state)
        .oneshot(bearer_request(
            "DELETE",
            format!("/clients/{client_id}/webhooks/{}", uuid::Uuid::new_v4()),
            &api_key,
            Body::empty(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = response_json(response).await;
    assert_eq!(
        json.get("code"),
        Some(&Value::String("RFA_JOB_NOT_FOUND".to_string()))
    );
}

#[tokio::test]
async fn given_existing_webhook_when_unregister_should_delete() {
    let Some((state, _db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;
    let api_key = create_api_key(state.clone(), client_id).await;

    let register = http::app(state.clone())
        .oneshot(bearer_request(
            "POST",
            format!("/clients/{client_id}/webhooks"),
            &api_key,
            Body::from(
                r#"{"url":"https://example.com/hook","events":["job_created"],"is_default":true}"#,
            ),
        ))
        .await
        .unwrap();
    let json = response_json(register).await;
    let webhook_id = json
        .get("webhook_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let delete = http::app(state)
        .oneshot(bearer_request(
            "DELETE",
            format!("/clients/{client_id}/webhooks/{webhook_id}"),
            &api_key,
            Body::empty(),
        ))
        .await
        .unwrap();

    assert_eq!(delete.status(), StatusCode::OK);
    let json = response_json(delete).await;
    assert_eq!(json.get("deleted"), Some(&Value::Bool(true)));
}
