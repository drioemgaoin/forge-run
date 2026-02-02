use crate::application::shared::api_key_helpers::{api_key_hash, api_key_prefix};
use crate::domain::value_objects::ids::ClientId;
use crate::interface::http::problem::{RFA_AUTH_INVALID_CREDENTIALS, RFA_INTERNAL, problem};
use crate::interface::http::state::AppState;
use crate::interface::http::trace::TraceId;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

/// Validates the API key (Bearer token) and injects the caller `ClientId` into the request.
///
/// Public endpoints are allowed through without a key (health checks and client signup).
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let trace_id = req.extensions().get::<TraceId>().map(|t| t.0.clone());
    // Step 1: allow unauthenticated public endpoints.
    let path = req.uri().path();
    let method = req.method().as_str();
    if path == "/health" || path == "/ready" || path == "/metrics" {
        return Ok(next.run(req).await);
    }
    if method == "POST" && path == "/clients" {
        return Ok(next.run(req).await);
    }
    if method == "POST" && path.starts_with("/clients/") && path.ends_with("/keys") {
        return Ok(next.run(req).await);
    }

    // Step 2: extract the Bearer token from the Authorization header.
    let header_value = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some(raw) = header_value
        .strip_prefix("Bearer ")
        .or_else(|| header_value.strip_prefix("bearer "))
    else {
        return Err(problem(
            StatusCode::UNAUTHORIZED,
            RFA_AUTH_INVALID_CREDENTIALS,
            Some("missing bearer token".to_string()),
            Some(path.to_string()),
            trace_id.clone(),
        ));
    };
    if raw.is_empty() {
        return Err(problem(
            StatusCode::UNAUTHORIZED,
            RFA_AUTH_INVALID_CREDENTIALS,
            Some("empty bearer token".to_string()),
            Some(path.to_string()),
            trace_id.clone(),
        ));
    }

    // Step 3: compute the stored lookup fields (prefix + hash).
    let prefix = api_key_prefix(raw);
    let hash = api_key_hash(raw);

    // Step 4: look up an active API key that matches this token.
    let key = state
        .ctx
        .repos
        .api_key
        .get_active_by_prefix_and_hash(&prefix, &hash)
        .await
        .map_err(|_| {
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                RFA_INTERNAL,
                Some("failed to verify api key".to_string()),
                Some(path.to_string()),
                trace_id.clone(),
            )
        })?;

    // Step 6: reject invalid keys; otherwise attach the client id for handlers.
    let Some(key) = key else {
        return Err(problem(
            StatusCode::UNAUTHORIZED,
            RFA_AUTH_INVALID_CREDENTIALS,
            Some("invalid api key".to_string()),
            Some(path.to_string()),
            trace_id,
        ));
    };

    req.extensions_mut().insert(ClientId(key.client_id));
    Ok(next.run(req).await)
}
