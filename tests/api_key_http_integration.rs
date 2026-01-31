use axum::body::Body;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use forge_run::application::context::AppContext;
use forge_run::config::Settings;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
use forge_run::infrastructure::db::repositories::Repositories;
use forge_run::infrastructure::db::stores::client_store::ClientStore;
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn setup_state() -> Option<(AppState, Arc<PostgresDatabase>)> {
    let url = test_db_url()?;
    let db = Arc::new(PostgresDatabase::connect(&url).await.ok()?);
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let ctx = AppContext::new(repos, Arc::new(lifecycle));
    let state = AppState {
        ctx: Arc::new(ctx),
        settings: Settings {
            server: forge_run::config::Server {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            db: forge_run::config::Db { url },
            redis: forge_run::config::Redis {
                url: "redis://127.0.0.1/".to_string(),
            },
        },
    };
    Some((state, db))
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
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

fn bearer_request(method: &str, uri: String, api_key: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {api_key}"))
        .body(body)
        .unwrap()
}

#[tokio::test]
async fn given_client_when_create_key_should_return_api_key() {
    let Some((state, db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;

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

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let api_key = json.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!api_key.is_empty());

    let client_store = ClientStorePostgres::new(db);
    client_store.delete(client_id).await.unwrap();
}

#[tokio::test]
async fn given_active_key_when_renew_with_auth_should_return_new_key() {
    let Some((state, db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;

    let create = http::app(state.clone())
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
    let created = response_json(create).await;
    let api_key = created
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let first_key_id = created.get("key_id").and_then(|v| v.as_str()).unwrap_or("");

    let renew = http::app(state)
        .oneshot(bearer_request(
            "POST",
            format!("/clients/{client_id}/keys/renew"),
            api_key,
            Body::empty(),
        ))
        .await
        .unwrap();

    assert_eq!(renew.status(), StatusCode::OK);
    let json = response_json(renew).await;
    let new_key = json.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
    let new_key_id = json.get("key_id").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!new_key.is_empty());
    assert_ne!(first_key_id, new_key_id);

    let client_store = ClientStorePostgres::new(db);
    client_store.delete(client_id).await.unwrap();
}

#[tokio::test]
async fn given_active_key_when_revoke_with_auth_should_return_revoked() {
    let Some((state, db)) = setup_state().await else {
        return;
    };
    let client_id = create_client_id(state.clone()).await;

    let create = http::app(state.clone())
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
    let created = response_json(create).await;
    let api_key = created
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let key_id = created.get("key_id").and_then(|v| v.as_str()).unwrap_or("");

    let revoke = http::app(state)
        .oneshot(bearer_request(
            "POST",
            format!("/clients/{client_id}/keys/revoke"),
            api_key,
            Body::from(format!(r#"{{"key_id":"{key_id}"}}"#)),
        ))
        .await
        .unwrap();

    assert_eq!(revoke.status(), StatusCode::OK);
    let json = response_json(revoke).await;
    assert_eq!(json.get("revoked"), Some(&Value::Bool(true)));

    let client_store = ClientStorePostgres::new(db);
    client_store.delete(client_id).await.unwrap();
}
