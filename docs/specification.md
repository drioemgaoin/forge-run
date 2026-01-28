## 1. What is this system?

- ForgeRun is a job execution API allowing users to submit jobs, track their execution, and receive events about their execution.
- ForgeRun will be used by me manually or by a simulator that I will implement that will simulate high traffic with different use cases.
- ForgeRun provides a report of all jobs with their final state and outcome.
- ForgeRun exists to help me practice Rust and cover all the concepts any real-world API needs developers to implement.

## 2. What problems does it solve?

ForgeRun solves the problem of not having a smooth and consistent way to execute jobs, track their execution, and retrieve their outcome. Right now, the job tracking and outcome collection is done manually and is prone to errors. It is impossible to do that manually at scale.

## 3. What are the non-goals?

ForgeRun will execute only simulated jobs meant to test the API’s behavior, not real workloads like image processing or heavy computation.
ForgeRun will be free to run as everything will be run locally or on machines I own.

## 4. Who are the users?

ForgeRun will be used by:

- me, manually
- a simulator that will execute different scenarios to test the API in a very granular way

## 5. What are the core concepts?

Job (includes type: instant or scheduled)

- Represents a unit of work to execute, track, and collect outcome
- Exists to describe the work to do
- Identified by a job id
- Changes over time

Event

- Represents a state change in the job execution
- Exists to notify and track any change in the job lifecycle
- Identified by (job id, timestamp, event name)
- Does not change over time

Report

- Represents the details of the job execution
- Exists to understand what happened to a job during its execution
- Identified by a job id (one report per job)
- Does not change over time

Client

- Represents the initiator of the job flow
- Exists to initiate job flow
- Identified by a client id
- Does not change over time

## 6. Job catalog

All jobs are synthetic. The API generates job definitions using the request timestamp as seed and fixed boundaries.

### Job Definition Generation Rules

- Client selects a job type (Instant or Deferred).
- API generates a job definition using the request timestamp as the random seed.
- Scenario is derived from `work_kind` or randomly chosen if not provided.
- Scenario controls: `duration_ms`, `should_fail`, and `payload_size`.
- Scenarios must be observable:
  - Logs include scenario, duration, and expected outcome.
  - Metrics include scenario counts and success/failure rates.
  - Events include scenario in the payload.
- `work_kind` is a required enum and maps to deterministic job behavior.

### Work Kind Catalog (deterministic mapping)

Format: `work_kind` → `duration_ms`, `should_fail`, `payload_size`, expected outcome.

- SUCCESS_FAST → 1_000, false, 4 KB, SUCCEEDED
- SUCCESS_NORMAL → 10_000, false, 16 KB, SUCCEEDED
- SUCCESS_SLOW → 90_000, false, 32 KB, SUCCEEDED
- FAIL_IMMEDIATE → 500, true, 1 KB, FAILED
- FAIL_AFTER_PROGRESS → 20_000, true, 8 KB, FAILED
- FAIL_AFTER_RETRYABLE → 5_000, true, 8 KB, FAILED (retryable)
- RUNS_LONG → 110_000, false, 32 KB, SUCCEEDED
- RUNS_OVER_TIMEOUT → max_runtime + 1_000, true, 8 KB, FAILED (timeout)
- CPU_BURST → 8_000, false, 4 KB, SUCCEEDED
- MEMORY_SPIKE → 12_000, false, 64 KB, SUCCEEDED
- IO_HEAVY → 15_000, false, 32 KB, SUCCEEDED
- MANY_SMALL_OUTPUTS → 9_000, false, 16 KB, SUCCEEDED
- LARGE_OUTPUT → 9_000, false, 256 KB, SUCCEEDED
- CANCEL_BEFORE_START → 5_000, false, 4 KB, CANCELED (when cancel issued in QUEUED)
- CANCEL_DURING_RUN → 10_000, false, 4 KB, CANCELED (when cancel issued in RUNNING)
- RETRY_ON_FAIL → 3_000, true, 4 KB, FAILED then SUCCEEDED on retry
- RETRY_LIMIT_REACHED → 3_000, true, 4 KB, FAILED after max retries
- DUPLICATE_SUBMIT_SAME_KEY → 2_000, false, 4 KB, SUCCEEDED (same job id)
- DUPLICATE_SUBMIT_DIFFERENT_KEY → 2_000, false, 4 KB, SUCCEEDED (distinct jobs)
- WEBHOOK_SUCCESS → 2_000, false, 4 KB, SUCCEEDED (webhook delivered)
- WEBHOOK_TIMEOUT → 2_000, false, 4 KB, SUCCEEDED (webhook retries)
- WEBHOOK_5XX → 2_000, false, 4 KB, SUCCEEDED (webhook retries)
- WEBHOOK_RETRIES_EXHAUSTED → 2_000, false, 4 KB, SUCCEEDED (delivery failed)
- WEBHOOK_SLOW_RECEIVER → 2_000, false, 4 KB, SUCCEEDED (delivery delayed)
- SCHEDULED_ON_TIME → 2_000, false, 4 KB, SUCCEEDED
- SCHEDULED_LATE_RECOVERY → 2_000, false, 4 KB, SUCCEEDED (enqueued after restart)
- SCHEDULED_FAR_FUTURE → 2_000, false, 4 KB, SUCCEEDED
- PAYLOAD_SMALL → 2_000, false, 1 KB, SUCCEEDED
- PAYLOAD_MEDIUM → 2_000, false, 16 KB, SUCCEEDED
- PAYLOAD_LARGE → 2_000, false, 256 KB, SUCCEEDED
- PAYLOAD_INVALID → 0, false, 0 KB, REJECTED (validation error)

**Instant Job**

- Purpose: run work asynchronously as soon as possible
- Definition fields: `work_kind`, `duration_ms`, `should_fail`, `payload_size`
- Generation rules:
  - `duration_ms`: 500–120_000 (slow jobs are allowed)
  - `should_fail`: true with configurable probability
  - `payload_size`: 1–256 KB
  - `work_kind`: chosen to cover success/failure/slow scenarios

**Deferred Job**

- Purpose: run work at a future time
- Definition fields: `work_kind`, `execution_at`, `duration_ms`, `should_fail`
- Generation rules:
  - `execution_at`: provided by user; must be in the future
  - `duration_ms`: 500–120_000 (slow jobs are allowed)
  - `should_fail`: true with configurable probability
  - `work_kind`: chosen to cover success/failure/slow scenarios

## 7. What can users do?

** Submit a job**

- User: Client
- Input:
  - type: type of job to executed coming from a catalog of predefined jobs
  - execution_at: None for instant, datetime for future execution
  - callback: optional webhook to be notified for each state change
- Output: job id
- Side effect: record the job

** Cancel a job**

- User: Client
- Input: id of the job
- Output: confirmation or not the job has been CANCELED or not
- Side effect: update the job

** Retry a job**

- User: Client
- Input: id of the job
- Output: confirmation or not the job has been retried or not
- Side effect: update the job

** Get a job**

- User: Client
- Input: id of the job
- Output: the job definition and final state
- Side effect: none

** Get a job report**

- User: Client
- Input: id of the job
- Output: the execution details of the job
- Side effect: none

** Create a client**

- User: Client
- Input: none
- Output: id of the client
- Side effect: record a client

** Create api key**

- User: Client
- Input: id of the client
- Output: api key
- Side effect: update the client record

** Renew api key**

- User: Client
- Input: id of the client
- Output: api key
- Side effect: update the client record

** Revoke api key**

- User: Client
- Input: id of the client
- Output: confirmation that the api key has been revoked or not
- Side effect: update the client record

### Idempotency

- If the same request is sent twice, it must not create a second job or a different result.
- Client can send an idempotency key on submit; if the key was already used, return the same job id.
- Cancel and retry are safe to repeat; repeated calls return the current state and do not emit new events.
- API key creation is safe to repeat; if a key already exists, return its metadata unless rotation is requested.

## 8. API Contract

Base:

- Base URL: `/v1`
- Content-Type: `application/json`
- Auth: API key in `Authorization: Bearer <api_key>`
- Idempotency: optional `Idempotency-Key` header on create actions

Endpoints:

**Create client**

- Method: `POST`
- Path: `/clients`
- Request: empty
- Response: `{ client_id }`
- Success: 201
- Errors: 500, 503

**Create API key**

- Method: `POST`
- Path: `/clients/{client_id}/keys`
- Request: `{ rotate?: boolean }`
- Response: `{ api_key, key_id, created_at, expires_at }`
- Success: 201
- Errors: 400, 401, 403, 404, 409, 500, 503

**Renew API key**

- Method: `POST`
- Path: `/clients/{client_id}/keys/renew`
- Request: `{}`
- Response: `{ api_key, key_id, created_at, expires_at }`
- Success: 200
- Errors: 400, 401, 403, 404, 409, 500, 503

**Revoke API key**

- Method: `POST`
- Path: `/clients/{client_id}/keys/revoke`
- Request: `{ key_id }`
- Response: `{ revoked: true }`
- Success: 200
- Errors: 400, 401, 403, 404, 409, 500, 503

**Submit job**

- Method: `POST`
- Path: `/jobs`
- Request:
  - `type`: enum (`EXECUTE`, `DEFERRED`, `EMIT_EVENT`)
  - `execution_at`: optional ISO-8601 UTC timestamp
  - `callback`: optional URL (must be https:// or http://)
  - `idempotency_key`: optional string (if not provided then header is used)
- Response: `{ job_id: string, state: string, created_at: string }`
- Success: 202
- Errors: 400, 401, 403, 409, 429, 500, 503

**Cancel job**

- Method: `POST`
- Path: `/jobs/{job_id}/cancel`
- Request: empty
- Response: `{ job_id: string, state: string, updated_at: string }`
- Success: 200
- Errors: 400, 401, 403, 404, 409, 500, 503

**Retry job**

- Method: `POST`
- Path: `/jobs/{job_id}/retry`
- Request: empty
- Response: `{ job_id: string, state: string, updated_at: string, attempt: number }`
- Success: 200
- Errors: 400, 401, 403, 404, 409, 500, 503

**Get job**

- Method: `GET`
- Path: `/jobs/{job_id}`
- Response:
  - `job_id`: string
  - `type`: enum (`EXECUTE`, `DEFERRED`, `EMIT_EVENT`)
  - `state`: enum (section 7)
  - `outcome`: enum (`SUCCESS`, `FAILED`, `CANCELED`)
  - `created_at`: string (UTC)
  - `execution_at`: string (UTC) or null
  - `updated_at`: string (UTC)
  - `attempt`: number
  - `callback`: string or null
- Success: 200
- Errors: 401, 403, 404, 500, 503

**Get job report**

- Method: `GET`
- Path: `/jobs/{job_id}/report`
- Response:
  - `job_id`: string
  - `outcome`: enum (`SUCCESS`, `FAILED`, `CANCELED`)
  - `events`: array of event objects (see Event model)
  - `started_at`: string (UTC)
  - `finished_at`: string (UTC)
  - `duration_ms`: number
- Success: 200
- Errors: 401, 403, 404, 500, 503

**Register webhook**

- Method: `POST`
- Path: `/webhooks`
- Request: `{ url: string, events: string[] }`
- Response: `{ webhook_id: string, created_at: string }`
- Success: 201
- Errors: 400, 401, 403, 409, 500, 503

**Unregister webhook**

- Method: `DELETE`
- Path: `/webhooks/{webhook_id}`
- Response: `{ deleted: true }`
- Success: 200
- Errors: 401, 403, 404, 500, 503

## 9. What states exist?

** Job State **

- CREATED — job definition generated (after request accepted).
- QUEUED — ready and waiting in the global queue, not assigned yet.
- ASSIGNED — claimed by a runner but not started yet.
- RUNNING — runner is executing.
- SUCCEEDED — finished successfully.
- FAILED — finished with error.
- CANCELED — canceled before completion.

States: CREATED, QUEUED, ASSIGNED, RUNNING, SUCCEEDED, FAILED, CANCELED
Initial: CREATED
Final: SUCCEEDED, FAILED, CANCELED
Allowed:

- CREATED -> QUEUED
- CREATED -> CANCELED
- QUEUED -> ASSIGNED
- QUEUED -> CANCELED
- ASSIGNED -> RUNNING
- ASSIGNED -> CANCELED
- RUNNING -> SUCCEEDED
- RUNNING -> FAILED
- RUNNING -> CANCELED
- FAILED -> QUEUED

Forbidden:

- CREATED -> CREATED
- CREATED -> ASSIGNED
- CREATED -> RUNNING
- CREATED -> SUCCEEDED
- CREATED -> FAILED
- QUEUED -> CREATED
- QUEUED -> QUEUED
- QUEUED -> RUNNING
- QUEUED -> SUCCEEDED
- QUEUED -> FAILED
- ASSIGNED -> CREATED
- ASSIGNED -> QUEUED
- ASSIGNED -> ASSIGNED
- ASSIGNED -> SUCCEEDED
- ASSIGNED -> FAILED
- RUNNING -> CREATED
- RUNNING -> QUEUED
- RUNNING -> ASSIGNED
- RUNNING -> RUNNING
- FAILED -> CREATED
- FAILED -> ASSIGNED
- FAILED -> RUNNING
- FAILED -> SUCCEEDED
- FAILED -> FAILED
- FAILED -> CANCELED
- SUCCEEDED -> CREATED
- SUCCEEDED -> QUEUED
- SUCCEEDED -> ASSIGNED
- SUCCEEDED -> RUNNING
- SUCCEEDED -> SUCCEEDED
- SUCCEEDED -> FAILED
- SUCCEEDED -> CANCELED
- CANCELED -> CREATED
- CANCELED -> QUEUED
- CANCELED -> ASSIGNED
- CANCELED -> RUNNING
- CANCELED -> SUCCEEDED
- CANCELED -> FAILED
- CANCELED -> CANCELED

## 10. Scheduling Semantics

- `execution_at` is in UTC (ISO-8601). If omitted, job is instant.
- A scheduled job stays in CREATED until its scheduled time, then moves to QUEUED.
- Jobs must never execute before `execution_at`.
- If `execution_at` is in the past, reject the request (400).
- If the system restarts, scheduled jobs are reloaded and evaluated at startup.
- If a scheduled time is missed (downtime), job is enqueued immediately on recovery.
- Scheduling precision: seconds; acceptable clock skew: ±1s.

## 11. Retry Policy

- Retries are allowed only for FAILED jobs.
- Max retries: 3 (configurable).
- Retry action moves job to QUEUED and increments attempt count.
- Backoff: exponential with jitter (configurable base and max).
- If max retries reached, job remains FAILED and emits a final event.

## 12. Queue/Worker Behavior

- Queue is global and FIFO by default; jobs are dequeued in creation order.
- Workers poll for QUEUED jobs and claim one at a time.
- Claiming moves job to ASSIGNED with a lease timeout.
- If lease expires without RUNNING, job returns to QUEUED.
- Workers must heartbeat while RUNNING.
- Each worker has a max concurrency of 1 (configurable).
- Queue is persisted to storage; in-memory queues are not allowed.
- Lease timeout: 30s; heartbeat interval: 5s.

## 13. What must never happen?

**Uniqueness**: a job id must never refer to two different jobs.
**State integrity**: a job cannot be in two states at once.
**Ordering**: a job's events must never show an impossible sequence.
**Ownership**: a client must never be able to see or modify another client's jobs.
**Accounting**: a job must never finish without an outcome and report.

## 14. What happens when things fail?

### Authorization/Auth

Most important failure: Missing API key

- Failure: request without API key
- Detection: auth layer sees missing/empty key
- Must do: reject with unauthorized response
- Must NOT do: process the request or create a job
- User impact: client receives 401/403 and no job is created

Second most important failure: API key invalid

- Failure: API key does not match any client
- Detection: lookup returns no client
- Must do: reject with unauthorized response
- Must NOT do: create or expose any job data
- User impact: client receives 401/403

Third most important failure: API key expired

- Failure: API key is expired
- Detection: expiry check fails
- Must do: reject with unauthorized response
- Must NOT do: accept the request
- User impact: client receives 401/403

Other failures:

- Client API key is revoked
- Client API key can not be refreshed
- Client API key can not be revoked
- Client is not allowed to access the specific job

### Input/Validation

Most important failure: Missing required fields

- Failure: required field missing (job type or schedule)
- Detection: request validation fails
- Must do: reject with validation error
- Must NOT do: enqueue or persist a partial job
- User impact: client receives 400 with error details

Second most important failure: Invalid job type

- Failure: job type not in catalog
- Detection: validation against catalog fails
- Must do: reject with validation error
- Must NOT do: create a job with an unknown type
- User impact: client receives 400 with error details

Third most important failure: Job scheduled in the past

- Failure: execution_at is earlier than now
- Detection: timestamp comparison fails
- Must do: reject with validation error
- Must NOT do: enqueue the job
- User impact: client receives 400 with error details

Other failures:

- Invalid timestamp format
- Unsupported callback URL (invalid scheme/host)
- Payload too large or malformed

### Storage/Database

Most important failure: Job creation write fails

- Failure: database write fails for job creation
- Detection: write returns error/timeout
- Must do: return failure and do not enqueue
- Must NOT do: report success or leave partial records
- User impact: client receives 500/503 and no job id

Second most important failure: Read fails when fetching job/report

- Failure: database read fails
- Detection: read returns error/timeout
- Must do: return failure without partial data
- Must NOT do: return stale or inconsistent data
- User impact: client receives 500/503

Third most important failure: Partial write

- Failure: job created but event/report not saved
- Detection: write succeeds for job but fails for related records
- Must do: mark job as failed/incomplete and record recovery needed
- Must NOT do: leave inconsistent records
- User impact: client receives failure or job marked failed

Other failures:

- Unique constraint violation (duplicate job id)
- Connection timeout or pool exhausted

### Queue/Assignment

Most important failure: Queue is full

- Failure: enqueue rejected due to capacity
- Detection: queue returns full/backpressure
- Must do: reject or retry later (explicitly)
- Must NOT do: drop the job silently
- User impact: client receives 429/503 and no job queued

Second most important failure: No workers available

- Failure: no capacity to assign a job
- Detection: scheduler finds zero available workers
- Must do: keep job queued
- Must NOT do: mark job RUNNING
- User impact: job remains QUEUED

Third most important failure: Worker claims job but fails before starting

- Failure: worker crashes after assignment
- Detection: assignment timeout/heartbeat failure
- Must do: re-queue the job
- Must NOT do: leave job stuck in ASSIGNED
- User impact: job returns to QUEUED

Other failures:

- Enqueue fails (queue unavailable or backlog too large)

### Execution/Runner

Most important failure: Runner crashes mid-run

- Failure: runner crashes mid-run
- Detection: heartbeat/timeout from runner
- Must do: mark job as FAILED and emit event
- Must NOT do: leave job stuck in RUNNING
- User impact: client sees FAILED state and can retry

Second most important failure: Job assigned but definition missing

- Failure: job definition not found at execution time
- Detection: lookup returns no job payload
- Must do: fail the job and emit event
- Must NOT do: run with empty or incorrect data
- User impact: job fails with error

Third most important failure: Execution exceeds max duration

- Failure: job exceeds runtime limit
- Detection: runtime timer expires
- Must do: stop execution and mark FAILED
- Must NOT do: keep job RUNNING indefinitely
- User impact: job fails with timeout reason

Other failures:

- Execution finishes but outcome/report cannot be generated
- Execution produces invalid or incomplete output

### State/Conflict (already done, duplicate, invalid transition)

Most important failure: Cancel called on a final job

- Failure: cancel requested after job completed
- Detection: current state is final
- Must do: return no-op with current state
- Must NOT do: change state or emit cancel event
- User impact: client sees job unchanged

Second most important failure: Retry called on a non-FAILED job

- Failure: retry requested for a job not in FAILED
- Detection: current state check
- Must do: reject or no-op with current state
- Must NOT do: change state
- User impact: client sees job unchanged

Third most important failure: Duplicate submit

- Failure: same request submitted twice
- Detection: idempotency key or duplicate detection
- Must do: return existing job id
- Must NOT do: create a second job
- User impact: client receives existing job id

Other failures:

- Invalid state transition attempted
- Concurrent updates cause conflicting state changes

### Notification/Webhooks

Most important failure: Webhook endpoint unreachable

- Failure: webhook endpoint unreachable
- Detection: connection error/timeout
- Must do: record failure and retry per policy
- Must NOT do: block job completion
- User impact: job completes but webhook delivery fails/retries

Second most important failure: Webhook URL invalid

- Failure: webhook URL invalid or missing
- Detection: validation fails
- Must do: reject webhook registration or ignore callback
- Must NOT do: attempt delivery
- User impact: client receives validation error or no callback

Third most important failure: Webhook timeout

- Failure: webhook call exceeds timeout
- Detection: timeout reached
- Must do: record failure and retry per policy
- Must NOT do: block job completion
- User impact: delivery delayed or fails after retries

Other failures:

- Webhook returns non‑2xx response
- Webhook delivery retried and still fails (max retries reached)

### Limits/Throttling

Most important failure: Rate limit exceeded

- Failure: rate limit exceeded
- Detection: limiter rejects request
- Must do: reject with rate limit response
- Must NOT do: enqueue or persist the job
- User impact: client receives 429 and must retry later

Second most important failure: Client request quota reached

- Failure: client exceeds maximum requests
- Detection: quota counter exceeded
- Must do: reject with limit response
- Must NOT do: accept the request
- User impact: client receives 429

Other failures:

- Maximum number of active jobs reached
- Queue capacity reached (if you treat it as a limit here instead of Queue/Assignment)

### Timeouts/Latency

Most important failure: Execution timeout exceeded

- Failure: execution timeout exceeded
- Detection: runtime timer expires
- Must do: mark job FAILED and emit timeout event
- Must NOT do: keep job RUNNING indefinitely
- User impact: client sees FAILED with timeout reason

Second most important failure: Webhook call exceeds timeout

- Failure: webhook call exceeds timeout
- Detection: timeout reached
- Must do: record failure and retry per policy
- Must NOT do: block job completion
- User impact: delivery delayed or fails after retries

Third most important failure: Database operation exceeds timeout

- Failure: database operation exceeds timeout
- Detection: DB client timeout
- Must do: return failure and avoid partial writes
- Must NOT do: report success
- User impact: client receives 500/503

Other failures:

- API request exceeds processing timeout

### Error Model and Codes

All error codes are prefixed with `RFA_` (RustForgeApi) and scoped by context.

Common codes:

- `RFA_REQUEST_MALFORMED`
- `RFA_REQUEST_UNSUPPORTED_MEDIA_TYPE`
- `RFA_REQUEST_METHOD_NOT_ALLOWED`
- `RFA_REQUEST_NOT_ACCEPTABLE`
- `RFA_REQUEST_PAYLOAD_TOO_LARGE`
- `RFA_REQUEST_RATE_LIMITED`
- `RFA_AUTH_INVALID_CREDENTIALS`
- `RFA_AUTH_TOKEN_EXPIRED`
- `RFA_AUTH_FORBIDDEN`
- `RFA_AUTH_API_KEY_DISABLED`
- `RFA_JOB_VALIDATION_FAILED`
- `RFA_JOB_NOT_FOUND`
- `RFA_JOB_CONFLICT`
- `RFA_EXEC_QUEUE_FULL`
- `RFA_EXEC_TIMEOUT`
- `RFA_EXEC_IDEMPOTENCY_CONFLICT`
- `RFA_EXEC_WORKER_UNAVAILABLE`
- `RFA_STORAGE_DB_ERROR`
- `RFA_STORAGE_UNAVAILABLE`
- `RFA_DEPENDENCY_TIMEOUT`
- `RFA_CONFIG_INVALID`
- `RFA_INTERNAL`

HTTP status categories (guidance):

1xx Informational (not used in this API):

| Error code | HTTP | Description                     | Actions           |
| ---------- | ---- | ------------------------------- | ----------------- |
| N/A        | 100  | Continue (not used).            | No action needed. |
| N/A        | 101  | Switching Protocols (not used). | No action needed. |

2xx Success:

| Error code | HTTP | Description                 | Actions                             |
| ---------- | ---- | --------------------------- | ----------------------------------- |
| N/A        | 200  | OK (common for reads).      | None.                               |
| N/A        | 201  | Created (create endpoints). | None.                               |
| N/A        | 202  | Accepted (async execution). | Poll status or wait for completion. |
| N/A        | 204  | No Content (optional).      | None.                               |

3xx Redirection (not used for API responses):

| Error code | HTTP | Description                    | Actions                       |
| ---------- | ---- | ------------------------------ | ----------------------------- |
| N/A        | 301  | Moved Permanently (not used).  | Check client/proxy config.    |
| N/A        | 302  | Found (not used).              | Check client/proxy config.    |
| N/A        | 304  | Not Modified (not used).       | Check caching/proxy behavior. |
| N/A        | 307  | Temporary Redirect (not used). | Check client/proxy config.    |

4xx Client errors (request issues you can fix):

| Error code                           | HTTP | Description                                                | Actions                                   |
| ------------------------------------ | ---- | ---------------------------------------------------------- | ----------------------------------------- |
| `RFA_REQUEST_MALFORMED`              | 400  | Malformed request (invalid JSON, missing required fields). | Fix request format; check schema.         |
| `RFA_REQUEST_UNSUPPORTED_MEDIA_TYPE` | 415  | Unsupported `Content-Type`.                                | Use `application/json`.                   |
| `RFA_REQUEST_METHOD_NOT_ALLOWED`     | 405  | HTTP method not supported on this endpoint.                | Use the documented method.                |
| `RFA_REQUEST_NOT_ACCEPTABLE`         | 406  | Client `Accept` header not supported.                      | Accept `application/json`.                |
| `RFA_REQUEST_PAYLOAD_TOO_LARGE`      | 413  | Request body exceeds size limits.                          | Reduce payload size.                      |
| `RFA_JOB_VALIDATION_FAILED`          | 400  | Business rules failed.                                     | Fix field values; retry.                  |
| `RFA_AUTH_INVALID_CREDENTIALS`       | 401  | Missing/invalid API key or token.                          | Provide valid credentials; refresh token. |
| `RFA_AUTH_TOKEN_EXPIRED`             | 401  | Token expired.                                             | Refresh token.                            |
| `RFA_AUTH_FORBIDDEN`                 | 403  | Authenticated but not allowed.                             | Request access or correct scopes.         |
| `RFA_AUTH_API_KEY_DISABLED`          | 403  | API key disabled or revoked.                               | Use another key.                          |
| `RFA_JOB_NOT_FOUND`                  | 404  | Job does not exist.                                        | Verify job ID.                            |
| `RFA_JOB_CONFLICT`                   | 409  | Job already exists or conflicting state.                   | Use idempotency or handle conflict.       |
| `RFA_EXEC_IDEMPOTENCY_CONFLICT`      | 409  | Idempotency key reused with different payload.             | Reuse same payload or new key.            |
| `RFA_REQUEST_RATE_LIMITED`           | 429  | Too many requests.                                         | Retry with backoff; reduce rate.          |

5xx Server errors (infra/internal issues):

| Error code                    | HTTP | Description                           | Actions                     |
| ----------------------------- | ---- | ------------------------------------- | --------------------------- |
| `RFA_INTERNAL`                | 500  | Unexpected server error.              | Retry later.                |
| `RFA_STORAGE_DB_ERROR`        | 503  | Database unavailable or query failed. | Retry later.                |
| `RFA_STORAGE_UNAVAILABLE`     | 503  | Required dependency unavailable.      | Retry later.                |
| `RFA_EXEC_QUEUE_FULL`         | 503  | Execution queue at capacity.          | Retry later; lower load.    |
| `RFA_EXEC_WORKER_UNAVAILABLE` | 503  | No workers available.                 | Retry later; scale workers. |
| `RFA_EXEC_TIMEOUT`            | 504  | Operation exceeded timeout.           | Retry later.                |
| `RFA_DEPENDENCY_TIMEOUT`      | 504  | Downstream dependency timed out.      | Retry later.                |
| `RFA_CONFIG_INVALID`          | 503  | Server misconfiguration.              | Fix config.                 |

Authoritative references:

```
RFC 9110: https://www.rfc-editor.org/rfc/rfc9110
RFC 6585: https://www.rfc-editor.org/rfc/rfc6585
RFC 7807: https://www.rfc-editor.org/rfc/rfc7807
```

Problem Details format (RFC 7807):

- `type`: URI identifying the error type
- `title`: short, human-readable summary
- `status`: HTTP status code
- `detail`: human-readable explanation
- `instance`: request identifier
- `code`: one of the `RFA_` error codes

## 15. What data must be stored?

** API key**

- Needed for client to use the api
- Deleted on client request
- If missing, return 403

** Job **

- Needed to track job lifecycle and answer status/report queries
- Deleted when retention period expires or on explicit delete (if allowed)
- If missing, return 404 and treat as unknown job
- Required fields: `job_id`, `type`, `state`, `outcome`, `attempt`, `created_at`, `updated_at`, `execution_at`, `callback`

** Event **

- Needed to reconstruct job lifecycle and timing
- Deleted with the job or after a shorter retention window
- If missing, job still works but history/auditing is incomplete
- Required fields: `event_id`, `job_id`, `event_name`, `prev_state`, `next_state`, `timestamp`

** Report **

- Needed to provide final outcome details
- Deleted with the job or after retention period
- If missing, job exists but report endpoint returns not found
- Required fields: `job_id`, `outcome`, `started_at`, `finished_at`, `duration_ms`, `events[]`

### Data Retention Rules

- Jobs: retain for 30 days after reaching a final state, then delete
- Events: retained with the job; delete when job is deleted
- Reports: retain with the job; delete when job is deleted
- API keys: retain while active; keep revoked keys for audit for 90 days

## 16. What must be observable?

### Required Log Fields

- request_id
- client_id
- job_id (when applicable)
- action name (submit/cancel/retry/get/report)
- current_state and next_state (for transitions)
- error_code and error_message (on failures)

### Required Metrics Names

- jobs_submitted_total
- jobs_queued_total
- jobs_running_total
- jobs_succeeded_total
- jobs_failed_total
- jobs_canceled_total
- job_queue_depth
- job_queue_wait_seconds
- job_execution_seconds
- webhook_deliveries_total
- webhook_failures_total

Metrics:

- Job submissions per minute
- Queue length and time in QUEUED/ASSIGNED
- Job execution duration
- Success vs failed vs canceled count
- Webhook delivery success/failure
- API request rate by endpoint and status code
- API latency p95/p99
- Job execution latency p95/p99
- Retry attempts count and success rate
- Idempotency dedupe hits vs new jobs
- Worker heartbeat/availability count
- Database error rate

Logs:

- Job state transitions with job id
- Job failures with reason
- Webhook delivery attempts with status
- Auth failures (client id, reason)

Traces:

- Submit job → enqueue → assign → run → report
- Retry flow for a failed job
- Webhook notification flow
- Cancel flow
- API key lifecycle flow (create/revoke)

## 17. What limits exist?

- Max requests per client per minute (hard, reject with 429)
- Max active jobs system-wide (hard, reject or queue)
- Max queue size (hard, reject enqueue)
- Max job payload size (hard, reject with 400)
- Max job execution time (hard, mark FAILED)
- Max webhook retries (hard, stop retrying)

## 18. What is configurable?

- Rate limits per client
- Max active jobs
- Queue size
- Job execution timeout
- Webhook timeout and retry policy
- Data retention period (jobs/events/reports)
- Allowed job types catalog

## 19. What does “done” mean?

- All core actions work (submit, cancel, retry, get, report, webhook)
- Job state machine enforced with allowed transitions only
- Failure behavior defined and implemented for each category in section 9
- Observability signals exist for submissions, executions, failures, webhooks
- Data persists across restart and report can be retrieved for completed jobs

## 20. Open questions

None. All open questions have been resolved in this version.
