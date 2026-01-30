// HTTP routes: API key management.

use crate::application::usecases::create_api_key::{CreateApiKeyCommand, CreateApiKeyUseCase};
use crate::application::usecases::renew_api_key::{RenewApiKeyCommand, RenewApiKeyUseCase};
use crate::application::usecases::revoke_api_key::{RevokeApiKeyCommand, RevokeApiKeyUseCase};
use crate::domain::value_objects::ids::ClientId;
use crate::infrastructure::db::postgres::api_key_store_postgres::ApiKeyStorePostgres;
use crate::interface::http::dto::api_key::{CreateApiKeyRequest, RevokeApiKeyRequest};
use crate::interface::http::state::AppState;
use axum::extract::{Path, State};
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
    Json(payload): Json<CreateApiKeyRequest>,
) -> Json<serde_json::Value> {
    // Step 1: build store + use case.
    let store = ApiKeyStorePostgres::new(state.db.clone());
    let usecase = CreateApiKeyUseCase::new(state.db.clone(), store);
    // Step 2: parse and validate the client id.
    let client_id = match uuid::Uuid::parse_str(&client_id) {
        Ok(id) => ClientId(id),
        Err(_) => return Json(serde_json::json!({ "error": "invalid client_id" })),
    };
    // Step 3: execute the use case.
    let result = usecase
        .execute(CreateApiKeyCommand {
            client_id,
            rotate: payload.rotate.unwrap_or(false),
        })
        .await;
    // Step 4: map the output to a JSON response.
    match result {
        Ok(out) => Json(serde_json::json!({
            "api_key": out.api_key,
            "key_id": out.key_id.to_string(),
            "created_at": out.created_at.format(&Rfc3339).unwrap_or_default(),
            "expires_at": out.expires_at.map(|t| t.format(&Rfc3339).unwrap_or_default()),
            "key_prefix": out.key_prefix,
        })),
        Err(_) => Json(serde_json::json!({ "error": "storage unavailable" })),
    }
}

/// Rotates the client's API key and returns the new key.
async fn renew_key(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Json<serde_json::Value> {
    // Step 1: build store + use case.
    let store = ApiKeyStorePostgres::new(state.db.clone());
    let usecase = RenewApiKeyUseCase::new(state.db.clone(), store);
    // Step 2: parse and validate the client id.
    let client_id = match uuid::Uuid::parse_str(&client_id) {
        Ok(id) => ClientId(id),
        Err(_) => return Json(serde_json::json!({ "error": "invalid client_id" })),
    };
    // Step 3: execute the use case.
    let result = usecase.execute(RenewApiKeyCommand { client_id }).await;
    // Step 4: map the output to a JSON response.
    match result {
        Ok(out) => Json(serde_json::json!({
            "api_key": out.api_key,
            "key_id": out.key_id.to_string(),
            "created_at": out.created_at.format(&Rfc3339).unwrap_or_default(),
            "expires_at": out.expires_at.map(|t| t.format(&Rfc3339).unwrap_or_default()),
            "key_prefix": out.key_prefix,
        })),
        Err(_) => Json(serde_json::json!({ "error": "storage unavailable" })),
    }
}

/// Revokes a specific API key for a client.
async fn revoke_key(
    State(state): State<AppState>,
    Path(_client_id): Path<String>,
    Json(payload): Json<RevokeApiKeyRequest>,
) -> Json<serde_json::Value> {
    // Step 1: build store + use case.
    let store = ApiKeyStorePostgres::new(state.db.clone());
    let usecase = RevokeApiKeyUseCase::new(state.db.clone(), store);
    // Step 2: parse and validate the key id.
    let key_id = match uuid::Uuid::parse_str(&payload.key_id) {
        Ok(id) => id,
        Err(_) => return Json(serde_json::json!({ "error": "invalid key_id" })),
    };
    // Step 3: execute the use case and return status.
    let result = usecase.execute(RevokeApiKeyCommand { key_id }).await;
    match result {
        Ok(out) => Json(serde_json::json!({ "revoked": out.revoked })),
        Err(_) => Json(serde_json::json!({ "error": "storage unavailable" })),
    }
}
