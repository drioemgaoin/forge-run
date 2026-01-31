# Error Reference (RFC 7807)

This API returns errors using **RFC 7807 Problem Details** with the media type
`application/problem+json`. Each error includes:

- `status`: HTTP status code
- `code`: stable application error code (`RFA_*`)
- `title`: short summary
- `detail`: human-friendly explanation (when available)
- `instance`: request path (when available)

Example response:

```json
{
  "type": "about:blank",
  "title": "Bad Request",
  "status": 400,
  "detail": "invalid job_id",
  "instance": "/jobs/not-a-uuid",
  "code": "RFA_REQUEST_MALFORMED",
  "trace_id": "2b11a6e8-7d0a-46c6-9b22-2a8b6a31f0b1"
}
```

## How to Read Errors

1. Check the `code` first (stable, machine-readable).
2. Use `status` to understand HTTP semantics.
3. Read `detail` for human guidance.
4. If the error is unexpected, contact support with:
   - `code`
   - `status`
   - `instance`
   - `trace_id` (from the error body or `X-Request-Id` header)
   - request id / timestamp (if available)

## Error Codes

| Code | HTTP | Meaning | What you can do |
|---|---:|---|---|
| RFA_REQUEST_MALFORMED | 400 | Request is invalid (bad JSON, invalid id, missing fields). | Fix the request format and retry. |
| RFA_REQUEST_UNSUPPORTED_MEDIA_TYPE | 415 | Unsupported `Content-Type`. | Use `application/json`. |
| RFA_REQUEST_METHOD_NOT_ALLOWED | 405 | Wrong HTTP method. | Use the documented method for this endpoint. |
| RFA_REQUEST_NOT_ACCEPTABLE | 406 | Unsupported `Accept` header. | Accept `application/json`. |
| RFA_REQUEST_PAYLOAD_TOO_LARGE | 413 | Request body too large. | Reduce payload size and retry. |
| RFA_REQUEST_RATE_LIMITED | 429 | Too many requests. | Retry with backoff and lower request rate. |
| RFA_AUTH_INVALID_CREDENTIALS | 401 | Missing or invalid API key. | Provide a valid API key. |
| RFA_AUTH_FORBIDDEN | 403 | Authenticated but not allowed. | Check permissions or request access. |
| RFA_JOB_VALIDATION_FAILED | 400 | Business rules failed. | Fix field values and retry. |
| RFA_JOB_NOT_FOUND | 404 | Job or resource not found. | Verify the id and retry. |
| RFA_JOB_CONFLICT | 409 | Conflict (e.g., invalid state transition). | Retry later or resolve conflict. |
| RFA_EXEC_IDEMPOTENCY_CONFLICT | 409 | Idempotency key mismatch or conflict. | Reuse the same payload or choose a new key. |
| RFA_STORAGE_DB_ERROR | 503 | Storage/database error. | Retry later; if persistent, contact support. |
| RFA_STORAGE_UNAVAILABLE | 503 | Dependency unavailable. | Retry later; if persistent, contact support. |
| RFA_DEPENDENCY_TIMEOUT | 504 | Downstream timeout. | Retry later; if persistent, contact support. |
| RFA_INTERNAL | 500 | Unexpected server error. | Retry later; contact support if repeated. |

## Support Guidance

When contacting support, provide:
- `code` and `status`
- `instance` path
- `trace_id` (from `X-Request-Id`)
- timestamp of the request
- a minimal example request

## Coverage Note

Most endpoints already return RFC 7807 errors. A global middleware normalizes any
remaining error responses into `application/problem+json` with a best-effort code
mapping based on status.
