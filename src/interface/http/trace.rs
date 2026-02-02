use axum::http::{HeaderValue, Request, header};
use axum::middleware::Next;
use axum::response::Response;
use metrics::{counter, histogram};
use tracing::info;

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

/// Emits a structured request log with the trace id, status, and latency.
pub async fn request_log_middleware(req: Request<axum::body::Body>, next: Next) -> Response {
    // Step 1: Capture request metadata and start the timer.
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let trace_id = req.extensions().get::<TraceId>().map(|t| t.0.clone());
    let start = std::time::Instant::now();

    // Step 2: Run the request.
    let response = next.run(req).await;

    // Step 3: Emit a structured log entry.
    let latency_ms = start.elapsed().as_millis() as u64;
    let method_label = match method.as_str() {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "HEAD" => "HEAD",
        "OPTIONS" => "OPTIONS",
        _ => "OTHER",
    };
    let status_code = response.status().as_u16();
    let status_label = match status_code {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "other",
    };
    counter!("http_requests_total", "method" => method_label, "status" => status_label)
        .increment(1);
    histogram!(
        "http_request_duration_ms",
        "method" => method_label,
        "status" => status_label
    )
    .record(latency_ms as f64);
    info!(
        trace_id = trace_id.as_deref().unwrap_or(""),
        method = %method,
        path = %path,
        status = status_code,
        latency_ms,
        "http_request"
    );

    response
}
