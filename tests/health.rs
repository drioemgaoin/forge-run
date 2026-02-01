use axum::body::Body;
use axum::http::{Request, StatusCode};
use forge_run::application::context::AppContext;
use forge_run::config::Settings;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use tower::util::ServiceExt;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn health_endpoint_works() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());
    let ctx = AppContext::new(repos, std::sync::Arc::new(lifecycle));
    let state = AppState {
        ctx: std::sync::Arc::new(ctx),
        settings: Settings {
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
