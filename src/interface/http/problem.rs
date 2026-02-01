use crate::interface::http::trace::TraceId;
use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// RFC 7807 Problem Details payload.
#[derive(Debug, Serialize)]
pub struct ProblemDetails {
    /// A URI reference that identifies the problem type.
    #[serde(rename = "type")]
    pub r#type: String,
    /// A short, human-readable summary of the problem type.
    pub title: String,
    /// The HTTP status code for this occurrence.
    pub status: u16,
    /// A human-readable explanation specific to this occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// A URI reference that identifies this specific occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// A stable, machine-readable application error code (RFA_...).
    pub code: String,
    /// A per-request trace id that can be used for support/debugging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

/// Build a Problem Details response with the correct content-type.
pub fn problem(
    status: StatusCode,
    code: &str,
    detail: Option<String>,
    instance: Option<String>,
    trace_id: Option<String>,
) -> Response {
    // Step 1: Build the problem payload.
    let payload = ProblemDetails {
        r#type: "about:blank".to_string(),
        title: status.canonical_reason().unwrap_or("Error").to_string(),
        status: status.as_u16(),
        detail,
        instance,
        code: code.to_string(),
        trace_id,
    };

    // Step 2: Convert to an HTTP response with JSON body.
    let mut response = (status, Json(payload)).into_response();

    // Step 3: Ensure RFC 7807 content type.
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );

    response
}

// Common RFA error codes (from specification).
pub const RFA_REQUEST_MALFORMED: &str = "RFA_REQUEST_MALFORMED";
pub const RFA_REQUEST_UNSUPPORTED_MEDIA_TYPE: &str = "RFA_REQUEST_UNSUPPORTED_MEDIA_TYPE";
pub const RFA_REQUEST_METHOD_NOT_ALLOWED: &str = "RFA_REQUEST_METHOD_NOT_ALLOWED";
pub const RFA_REQUEST_NOT_ACCEPTABLE: &str = "RFA_REQUEST_NOT_ACCEPTABLE";
pub const RFA_REQUEST_PAYLOAD_TOO_LARGE: &str = "RFA_REQUEST_PAYLOAD_TOO_LARGE";
pub const RFA_REQUEST_RATE_LIMITED: &str = "RFA_REQUEST_RATE_LIMITED";
pub const RFA_AUTH_INVALID_CREDENTIALS: &str = "RFA_AUTH_INVALID_CREDENTIALS";
pub const RFA_AUTH_FORBIDDEN: &str = "RFA_AUTH_FORBIDDEN";
pub const RFA_JOB_VALIDATION_FAILED: &str = "RFA_JOB_VALIDATION_FAILED";
pub const RFA_JOB_NOT_FOUND: &str = "RFA_JOB_NOT_FOUND";
pub const RFA_JOB_CONFLICT: &str = "RFA_JOB_CONFLICT";
pub const RFA_EXEC_SCHEDULE_FULL: &str = "RFA_EXEC_SCHEDULE_FULL";
pub const RFA_EXEC_IDEMPOTENCY_CONFLICT: &str = "RFA_EXEC_IDEMPOTENCY_CONFLICT";
pub const RFA_STORAGE_DB_ERROR: &str = "RFA_STORAGE_DB_ERROR";
pub const RFA_STORAGE_UNAVAILABLE: &str = "RFA_STORAGE_UNAVAILABLE";
pub const RFA_DEPENDENCY_TIMEOUT: &str = "RFA_DEPENDENCY_TIMEOUT";
pub const RFA_INTERNAL: &str = "RFA_INTERNAL";

/// Convert a plain error response into RFC 7807 if needed.
pub fn problem_from_status(
    status: StatusCode,
    instance: Option<String>,
    trace_id: Option<String>,
) -> Response {
    let code = match status {
        StatusCode::BAD_REQUEST => RFA_REQUEST_MALFORMED,
        StatusCode::UNAUTHORIZED => RFA_AUTH_INVALID_CREDENTIALS,
        StatusCode::FORBIDDEN => RFA_AUTH_FORBIDDEN,
        StatusCode::NOT_FOUND => RFA_JOB_NOT_FOUND,
        StatusCode::METHOD_NOT_ALLOWED => RFA_REQUEST_METHOD_NOT_ALLOWED,
        StatusCode::NOT_ACCEPTABLE => RFA_REQUEST_NOT_ACCEPTABLE,
        StatusCode::PAYLOAD_TOO_LARGE => RFA_REQUEST_PAYLOAD_TOO_LARGE,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => RFA_REQUEST_UNSUPPORTED_MEDIA_TYPE,
        StatusCode::TOO_MANY_REQUESTS => RFA_REQUEST_RATE_LIMITED,
        StatusCode::SERVICE_UNAVAILABLE => RFA_STORAGE_UNAVAILABLE,
        StatusCode::GATEWAY_TIMEOUT => RFA_DEPENDENCY_TIMEOUT,
        _ => RFA_INTERNAL,
    };
    problem(status, code, None, instance, trace_id)
}

/// Middleware that normalizes error responses to RFC 7807 format.
pub async fn problem_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let path = req.uri().path().to_string();
    let trace_id = req.extensions().get::<TraceId>().map(|t| t.0.clone());
    let mut response = next.run(req).await;
    let status = response.status();
    let is_error = status.is_client_error() || status.is_server_error();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if is_error && !content_type.starts_with("application/problem+json") {
        response = problem_from_status(status, Some(path), trace_id);
    }

    response
}
