use axum::http::{HeaderValue, Request, header};
use axum::middleware::Next;
use axum::response::Response;

/// A per-request trace identifier used for support and debugging.
#[derive(Debug, Clone)]
pub struct TraceId(pub String);

/// Injects a trace id into request extensions and response headers.
pub async fn trace_id_middleware(req: Request<axum::body::Body>, next: Next) -> Response {
    // Step 1: Reuse a client-provided id or generate a new one.
    let trace_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let trace_id = TraceId(trace_id);

    // Step 2: Attach the trace id to the request for downstream handlers.
    let mut req = req;
    req.extensions_mut().insert(trace_id.clone());

    // Step 3: Run the request.
    let mut response = next.run(req).await;

    // Step 4: Add the trace id to the response header and extensions.
    if let Ok(value) = HeaderValue::from_str(&trace_id.0) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-request-id"), value);
    }
    response.extensions_mut().insert(trace_id);

    response
}
