// HTTP routes: webhook registration.

use crate::application::usecases::register_webhook::{
    RegisterWebhookCommand, RegisterWebhookError, RegisterWebhookUseCase,
};
use crate::application::usecases::unregister_webhook::{
    UnregisterWebhookCommand, UnregisterWebhookError, UnregisterWebhookUseCase,
};
use crate::interface::http::dto::webhook::{
    RegisterWebhookRequest, RegisterWebhookResponse, UnregisterWebhookResponse,
};
use crate::interface::http::problem::{
    RFA_JOB_CONFLICT, RFA_JOB_NOT_FOUND, RFA_REQUEST_MALFORMED, RFA_STORAGE_DB_ERROR, problem,
};
use crate::interface::http::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use time::format_description::well_known::Rfc3339;

/// Builds webhook routes.
pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/webhooks", post(register_webhook))
        .route("/webhooks/:webhook_id", delete(unregister_webhook))
}

/// Registers a webhook.
async fn register_webhook(
    State(state): State<AppState>,
    Json(payload): Json<RegisterWebhookRequest>,
) -> Response {
    // Step 1: Validate payload basics.
    if payload.url.trim().is_empty() {
        return problem(
            StatusCode::BAD_REQUEST,
            RFA_REQUEST_MALFORMED,
            Some("url is required".to_string()),
            None,
        );
    }
    if payload.events.is_empty() {
        return problem(
            StatusCode::BAD_REQUEST,
            RFA_REQUEST_MALFORMED,
            Some("events list is required".to_string()),
            None,
        );
    }

    // Step 2: Execute the use case.
    let result = RegisterWebhookUseCase::execute(
        &state.ctx,
        RegisterWebhookCommand {
            url: payload.url.clone(),
            events: payload.events.clone(),
        },
    )
    .await;

    // Step 3: Map output to HTTP response.
    match result {
        Ok(out) => {
            let response = RegisterWebhookResponse {
                webhook_id: out.webhook_id.to_string(),
                created_at: out.created_at.format(&Rfc3339).unwrap_or_default(),
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(RegisterWebhookError::Conflict) => problem(
            StatusCode::CONFLICT,
            RFA_JOB_CONFLICT,
            Some("webhook already exists".to_string()),
            None,
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
        ),
    }
}

/// Unregisters a webhook.
async fn unregister_webhook(
    State(state): State<AppState>,
    Path(webhook_id): Path<String>,
) -> Response {
    // Step 1: Parse webhook id.
    let webhook_id = match uuid::Uuid::parse_str(&webhook_id) {
        Ok(id) => id,
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid webhook_id".to_string()),
                None,
            );
        }
    };

    // Step 2: Execute the use case.
    let result =
        UnregisterWebhookUseCase::execute(&state.ctx, UnregisterWebhookCommand { webhook_id })
            .await;

    // Step 3: Map output to HTTP response.
    match result {
        Ok(out) => {
            let response = UnregisterWebhookResponse {
                deleted: out.deleted,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(UnregisterWebhookError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("webhook not found".to_string()),
            None,
        ),
        Err(UnregisterWebhookError::Storage(_)) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
        ),
    }
}
