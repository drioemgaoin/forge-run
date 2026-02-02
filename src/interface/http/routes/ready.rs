use crate::interface::http::state::AppState;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ReadyResponse {
    status: &'static str,
}

/// Builds the readiness route.
pub fn router() -> Router<AppState> {
    Router::new().route("/ready", get(ready))
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let result = state.ctx.repos.execute("SELECT 1").await;
    match result {
        Ok(_) => (StatusCode::OK, Json(ReadyResponse { status: "ready" })),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "not_ready",
            }),
        ),
    }
}
