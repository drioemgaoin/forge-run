use axum::body::Body;
use axum::http::{Request, StatusCode};
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use forge_run::infrastructure::db::database::{Database, DatabaseError};
use forge_run::config::Settings;
use async_trait::async_trait;
use tower::util::ServiceExt;

struct DummyDb;

#[async_trait]
impl Database for DummyDb {
    async fn execute(&self, _query: &str) -> Result<u64, DatabaseError> {
        Ok(0)
    }
}

#[tokio::test]
async fn health_endpoint_works() {
    let state = AppState {
        db: std::sync::Arc::new(DummyDb),
        settings: Settings {
            server: forge_run::config::Server {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            db: forge_run::config::Db {
                url: "postgres://localhost/forge_run_test".to_string(),
            },
            redis: forge_run::config::Redis {
                url: "redis://127.0.0.1/".to_string(),
            },
        },
    };

    let response = http::app(state)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
