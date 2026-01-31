// HTTP routes: client creation.

use crate::interface::http::problem::{RFA_STORAGE_DB_ERROR, problem};
use crate::interface::http::state::AppState;
use crate::interface::http::trace::TraceId;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::post};

/// Builds the client routes (currently only client creation).
pub fn router() -> Router<AppState> {
    Router::new().route("/clients", post(create_client))
}

/// Creates a new client and returns its identifier.
///
/// This is a public endpoint used during onboarding.
async fn create_client(
    state: axum::extract::State<AppState>,
    axum::extract::Extension(trace_id): axum::extract::Extension<TraceId>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: Run the use case.
    let result =
        crate::application::usecases::create_client::CreateClientUseCase::execute(&state.ctx).await;

    // Step 2: Map output to HTTP response.
    match result {
        Ok(client) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "client_id": client.id.0.to_string() })),
        )
            .into_response(),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}
