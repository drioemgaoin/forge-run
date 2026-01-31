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
}

/// Build a Problem Details response with the correct content-type.
pub fn problem(
    status: StatusCode,
    code: &str,
    detail: Option<String>,
    instance: Option<String>,
) -> Response {
    // Step 1: Build the problem payload.
    let payload = ProblemDetails {
        r#type: "about:blank".to_string(),
        title: status.canonical_reason().unwrap_or("Error").to_string(),
        status: status.as_u16(),
        detail,
        instance,
        code: code.to_string(),
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
pub const RFA_AUTH_INVALID_CREDENTIALS: &str = "RFA_AUTH_INVALID_CREDENTIALS";
pub const RFA_AUTH_FORBIDDEN: &str = "RFA_AUTH_FORBIDDEN";
pub const RFA_JOB_VALIDATION_FAILED: &str = "RFA_JOB_VALIDATION_FAILED";
pub const RFA_JOB_NOT_FOUND: &str = "RFA_JOB_NOT_FOUND";
pub const RFA_JOB_CONFLICT: &str = "RFA_JOB_CONFLICT";
pub const RFA_EXEC_IDEMPOTENCY_CONFLICT: &str = "RFA_EXEC_IDEMPOTENCY_CONFLICT";
pub const RFA_STORAGE_DB_ERROR: &str = "RFA_STORAGE_DB_ERROR";
pub const RFA_INTERNAL: &str = "RFA_INTERNAL";
