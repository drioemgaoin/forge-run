// HTTP routes: API key management.

use crate::application::usecases::create_api_key::CreateApiKeyCommand;
use crate::application::usecases::renew_api_key::RenewApiKeyCommand;
use crate::application::usecases::revoke_api_key::RevokeApiKeyCommand;
use crate::domain::value_objects::ids::ClientId;
use crate::interface::http::dto::api_key::{CreateApiKeyRequest, RevokeApiKeyRequest};
use crate::interface::http::problem::{
    RFA_JOB_NOT_FOUND, RFA_REQUEST_MALFORMED, RFA_STORAGE_DB_ERROR, problem,
};
use crate::interface::http::state::AppState;
use crate::interface::http::trace::TraceId;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::post};
use time::format_description::well_known::Rfc3339;

/// Builds the API key management routes for clients.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/clients/:client_id/keys", post(create_key))
        .route("/clients/:client_id/keys/renew", post(renew_key))
        .route("/clients/:client_id/keys/revoke", post(revoke_key))
}

/// Creates a new API key for a client.
///
/// Returns the raw key once (for the caller to store safely).
async fn create_key(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
    Extension(trace_id): Extension<TraceId>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: parse and validate the client id.
    let client_id = match uuid::Uuid::parse_str(&client_id) {
        Ok(id) => ClientId(id),
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid client_id".to_string()),
                None,
                trace_id,
            );
        }
    };
    // Step 2: execute the use case.
    let result = crate::application::usecases::create_api_key::CreateApiKeyUseCase::execute(
        &state.ctx,
        CreateApiKeyCommand {
            client_id,
            rotate: payload.rotate.unwrap_or(false),
        },
    )
    .await;
    // Step 3: map the output to a JSON response.
    match result {
        Ok(out) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "api_key": out.api_key,
                "key_id": out.key_id.to_string(),
                "created_at": out.created_at.format(&Rfc3339).unwrap_or_default(),
                "expires_at": out.expires_at.map(|t| t.format(&Rfc3339).unwrap_or_default()),
                "key_prefix": out.key_prefix,
            })),
        )
            .into_response(),
        Err(crate::application::shared::api_key_types::ApiKeyUseCaseError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("client not found".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}

/// Rotates the client's API key and returns the new key.
async fn renew_key(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
    Extension(trace_id): Extension<TraceId>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: parse and validate the client id.
    let client_id = match uuid::Uuid::parse_str(&client_id) {
        Ok(id) => ClientId(id),
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid client_id".to_string()),
                None,
                trace_id,
            );
        }
    };
    // Step 2: execute the use case.
    let result = crate::application::usecases::renew_api_key::RenewApiKeyUseCase::execute(
        &state.ctx,
        RenewApiKeyCommand { client_id },
    )
    .await;
    // Step 3: map the output to a JSON response.
    match result {
        Ok(out) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "api_key": out.api_key,
                "key_id": out.key_id.to_string(),
                "created_at": out.created_at.format(&Rfc3339).unwrap_or_default(),
                "expires_at": out.expires_at.map(|t| t.format(&Rfc3339).unwrap_or_default()),
                "key_prefix": out.key_prefix,
            })),
        )
            .into_response(),
        Err(crate::application::shared::api_key_types::ApiKeyUseCaseError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("client not found".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}

/// Revokes a specific API key for a client.
async fn revoke_key(
    State(state): State<AppState>,
    Path(_client_id): Path<String>,
    Extension(trace_id): Extension<TraceId>,
    Json(payload): Json<RevokeApiKeyRequest>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: parse and validate the key id.
    let key_id = match uuid::Uuid::parse_str(&payload.key_id) {
        Ok(id) => id,
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid key_id".to_string()),
                None,
                trace_id,
            );
        }
    };
    // Step 2: execute the use case and return status.
    let result = crate::application::usecases::revoke_api_key::RevokeApiKeyUseCase::execute(
        &state.ctx,
        RevokeApiKeyCommand { key_id },
    )
    .await;
    match result {
        Ok(out) => (
            StatusCode::OK,
            Json(serde_json::json!({ "revoked": out.revoked })),
        )
            .into_response(),
        Err(crate::application::shared::api_key_types::ApiKeyUseCaseError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("key not found".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}
