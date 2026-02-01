// HTTP routes: submit/cancel/retry/get jobs.

use crate::application::usecases::cancel_job::{CancelJobError, CancelJobUseCase};
use crate::application::usecases::get_job::{GetJobError, GetJobUseCase};
use crate::application::usecases::retry_job::{RetryJobError, RetryJobUseCase};
use crate::application::usecases::submit_job::{SubmitJobCommand, SubmitJobUseCase};
use crate::domain::entities::job::{JobOutcome, JobType};
use crate::domain::value_objects::ids::{ClientId, JobId};
use crate::domain::value_objects::timestamps::Timestamp;
use crate::interface::http::dto::job::{
    JobResponse, JobStateResponse, SubmitJobRequest, SubmitJobResponse,
};
use crate::interface::http::problem::{
    RFA_EXEC_IDEMPOTENCY_CONFLICT, RFA_EXEC_SCHEDULE_FULL, RFA_JOB_CONFLICT, RFA_JOB_NOT_FOUND,
    RFA_JOB_VALIDATION_FAILED, RFA_REQUEST_MALFORMED, RFA_STORAGE_DB_ERROR, problem,
};
use crate::interface::http::state::AppState;
use crate::interface::http::trace::TraceId;
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Builds job lifecycle routes.
pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/jobs", post(submit_job))
        .route("/jobs/:job_id", get(get_job))
        .route("/jobs/:job_id/cancel", post(cancel_job))
        .route("/jobs/:job_id/retry", post(retry_job))
}

/// Submits a job and returns the job id and current state.
async fn submit_job(
    State(state): State<AppState>,
    Extension(client_id): Extension<ClientId>,
    Extension(trace_id): Extension<TraceId>,
    headers: HeaderMap,
    Json(payload): Json<SubmitJobRequest>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: Parse and validate the job type.
    let job_type = payload.r#type.to_uppercase();
    let is_deferred = job_type == "DEFERRED";
    let is_execute = job_type == "EXECUTE";
    if !(is_deferred || is_execute) {
        return problem(
            StatusCode::BAD_REQUEST,
            RFA_REQUEST_MALFORMED,
            Some("invalid job type".to_string()),
            None,
            trace_id,
        );
    }

    // Step 2: Parse execution_at when provided.
    let execution_at = match payload.execution_at.as_deref() {
        Some(raw) => match OffsetDateTime::parse(raw, &Rfc3339) {
            Ok(dt) => Some(Timestamp::from(dt)),
            Err(_) => {
                return problem(
                    StatusCode::BAD_REQUEST,
                    RFA_REQUEST_MALFORMED,
                    Some("invalid execution_at timestamp".to_string()),
                    None,
                    trace_id.clone(),
                );
            }
        },
        None => None,
    };

    // Step 3: Validate deferred/execute constraints.
    if is_deferred && execution_at.is_none() {
        return problem(
            StatusCode::BAD_REQUEST,
            RFA_JOB_VALIDATION_FAILED,
            Some("execution_at is required for deferred jobs".to_string()),
            None,
            trace_id.clone(),
        );
    }
    if is_execute && execution_at.is_some() {
        return problem(
            StatusCode::BAD_REQUEST,
            RFA_JOB_VALIDATION_FAILED,
            Some("execution_at not allowed for execute jobs".to_string()),
            None,
            trace_id.clone(),
        );
    }

    // Step 4: Resolve idempotency key (body takes precedence over header).
    let header_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let idempotency_key = match (payload.idempotency_key.clone(), header_key) {
        (Some(body_key), Some(header_key)) if body_key != header_key => {
            return problem(
                StatusCode::CONFLICT,
                RFA_EXEC_IDEMPOTENCY_CONFLICT,
                Some("idempotency key mismatch".to_string()),
                None,
                trace_id.clone(),
            );
        }
        (Some(body_key), _) => Some(body_key),
        (None, header_key) => header_key,
    };

    // Step 5: Validate work_kind presence.
    let work_kind = match payload.work_kind.clone() {
        Some(kind) => Some(kind),
        None => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_JOB_VALIDATION_FAILED,
                Some("work_kind is required".to_string()),
                None,
                trace_id.clone(),
            );
        }
    };

    // Step 6: Execute the submit use case.
    let result = SubmitJobUseCase::execute(
        &state.ctx,
        SubmitJobCommand {
            client_id,
            execution_at,
            callback_url: payload.callback.clone(),
            work_kind,
            idempotency_key,
        },
    )
    .await;

    // Step 7: Map output to HTTP response.
    match result {
        Ok(out) => {
            let response = SubmitJobResponse {
                job_id: out.job.id.0.to_string(),
                state: out.job.state.as_str().to_string(),
                created_at: out
                    .job
                    .created_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
            };
            (StatusCode::ACCEPTED, Json(response)).into_response()
        }
        Err(crate::domain::services::job_lifecycle::JobLifecycleError::Validation(
            crate::domain::entities::job::JobValidationError::ScheduleWindowFull,
        )) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_EXEC_SCHEDULE_FULL,
            Some("execution window is at capacity".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(crate::domain::services::job_lifecycle::JobLifecycleError::Validation(_)) => problem(
            StatusCode::BAD_REQUEST,
            RFA_JOB_VALIDATION_FAILED,
            Some("job validation failed".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(crate::domain::services::job_lifecycle::JobLifecycleError::Storage(msg))
            if msg.contains("idempotency") =>
        {
            problem(
                StatusCode::CONFLICT,
                RFA_EXEC_IDEMPOTENCY_CONFLICT,
                Some(msg),
                None,
                trace_id.clone(),
            )
        }
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}

/// Cancels a job and returns its updated state.
async fn cancel_job(
    State(state): State<AppState>,
    Extension(trace_id): Extension<TraceId>,
    Path(job_id): Path<String>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: Parse the job id.
    let job_id = match uuid::Uuid::parse_str(&job_id) {
        Ok(id) => JobId(id),
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid job_id".to_string()),
                None,
                trace_id,
            );
        }
    };

    // Step 2: Execute the cancel use case.
    let result = CancelJobUseCase::execute(&state.ctx, job_id).await;

    // Step 3: Map output to HTTP response.
    match result {
        Ok(out) => {
            let response = JobStateResponse {
                job_id: out.job.id.0.to_string(),
                state: out.job.state.as_str().to_string(),
                updated_at: out
                    .job
                    .updated_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
                attempt: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(CancelJobError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("job not found".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(CancelJobError::Transition(_)) => problem(
            StatusCode::CONFLICT,
            RFA_JOB_CONFLICT,
            Some("invalid state transition".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(CancelJobError::Storage(_)) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}

/// Retries a failed job and returns its updated state.
async fn retry_job(
    State(state): State<AppState>,
    Extension(trace_id): Extension<TraceId>,
    Path(job_id): Path<String>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: Parse the job id.
    let job_id = match uuid::Uuid::parse_str(&job_id) {
        Ok(id) => JobId(id),
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid job_id".to_string()),
                None,
                trace_id,
            );
        }
    };

    // Step 2: Execute the retry use case.
    let result = RetryJobUseCase::execute(&state.ctx, job_id).await;

    // Step 3: Map output to HTTP response.
    match result {
        Ok(out) => {
            let response = JobStateResponse {
                job_id: out.job.id.0.to_string(),
                state: out.job.state.as_str().to_string(),
                updated_at: out
                    .job
                    .updated_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
                attempt: Some(out.job.attempt as i64),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(RetryJobError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("job not found".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(RetryJobError::InvalidState)
        | Err(RetryJobError::RetryLimitReached)
        | Err(RetryJobError::Transition(_)) => problem(
            StatusCode::CONFLICT,
            RFA_JOB_CONFLICT,
            Some("invalid state transition".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(RetryJobError::Storage(_)) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}

/// Fetches a job and returns its details.
async fn get_job(
    State(state): State<AppState>,
    Extension(trace_id): Extension<TraceId>,
    Path(job_id): Path<String>,
) -> Response {
    let trace_id = Some(trace_id.0.clone());
    // Step 1: Parse the job id.
    let job_id = match uuid::Uuid::parse_str(&job_id) {
        Ok(id) => JobId(id),
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid job_id".to_string()),
                None,
                trace_id,
            );
        }
    };

    // Step 2: Execute the get use case.
    let result = GetJobUseCase::execute(&state.ctx, job_id).await;

    // Step 3: Map output to HTTP response.
    match result {
        Ok(job) => {
            let response = JobResponse {
                job_id: job.id.0.to_string(),
                r#type: map_job_type(job.job_type),
                state: job.state.as_str().to_string(),
                outcome: job.outcome.map(map_outcome),
                created_at: job
                    .created_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
                execution_at: job
                    .executed_at
                    .map(|t| t.as_inner().format(&Rfc3339).unwrap_or_default()),
                updated_at: job
                    .updated_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
                attempt: job.attempt as i64,
                callback: job.callback_url.clone(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(GetJobError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("job not found".to_string()),
            None,
            trace_id.clone(),
        ),
        Err(GetJobError::Storage(_)) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
            trace_id,
        ),
    }
}

fn map_job_type(job_type: JobType) -> String {
    match job_type {
        JobType::Instant => "EXECUTE".to_string(),
        JobType::Deferred => "DEFERRED".to_string(),
    }
}

fn map_outcome(outcome: JobOutcome) -> String {
    match outcome {
        JobOutcome::Success => "SUCCESS".to_string(),
        JobOutcome::Failed => "FAILED".to_string(),
        JobOutcome::Canceled => "CANCELED".to_string(),
    }
}
