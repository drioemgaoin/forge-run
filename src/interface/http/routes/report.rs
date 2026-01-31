// HTTP routes: job reports.

use crate::application::usecases::get_report::{GetReportError, GetReportUseCase};
use crate::domain::value_objects::ids::JobId;
use crate::interface::http::dto::report::{ReportEventResponse, ReportResponse};
use crate::interface::http::problem::{
    RFA_JOB_NOT_FOUND, RFA_REQUEST_MALFORMED, RFA_STORAGE_DB_ERROR, problem,
};
use crate::interface::http::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use time::format_description::well_known::Rfc3339;

/// Builds report routes.
pub fn router() -> axum::Router<AppState> {
    axum::Router::new().route("/jobs/:job_id/report", get(get_report))
}

/// Fetches a job report.
async fn get_report(State(state): State<AppState>, Path(job_id): Path<String>) -> Response {
    // Step 1: Parse the job id.
    let job_id = match uuid::Uuid::parse_str(&job_id) {
        Ok(id) => JobId(id),
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                RFA_REQUEST_MALFORMED,
                Some("invalid job_id".to_string()),
                None,
            );
        }
    };

    // Step 2: Execute the report use case.
    let result = GetReportUseCase::execute(&state.ctx, job_id).await;

    // Step 3: Map output to HTTP response.
    match result {
        Ok(report) => {
            let events = report
                .events
                .iter()
                .map(|e| ReportEventResponse {
                    event_name: map_event_name(e.event_name),
                    prev_state: e.prev_state.as_str().to_string(),
                    next_state: e.next_state.as_str().to_string(),
                    timestamp: e.timestamp.as_inner().format(&Rfc3339).unwrap_or_default(),
                })
                .collect::<Vec<_>>();

            let response = ReportResponse {
                job_id: report.job_id.0.to_string(),
                outcome: format!("{:?}", report.outcome).to_uppercase(),
                events,
                started_at: report
                    .started_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
                finished_at: report
                    .finished_at
                    .as_inner()
                    .format(&Rfc3339)
                    .unwrap_or_default(),
                duration_ms: report.duration_ms,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(GetReportError::NotFound) => problem(
            StatusCode::NOT_FOUND,
            RFA_JOB_NOT_FOUND,
            Some("report not found".to_string()),
            None,
        ),
        Err(GetReportError::Storage(_)) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            RFA_STORAGE_DB_ERROR,
            Some("storage unavailable".to_string()),
            None,
        ),
    }
}

fn map_event_name(event_name: crate::domain::entities::event::EventName) -> String {
    match event_name {
        crate::domain::entities::event::EventName::JobCreated => "job_created".to_string(),
        crate::domain::entities::event::EventName::JobQueued => "job_queued".to_string(),
        crate::domain::entities::event::EventName::JobAssigned => "job_assigned".to_string(),
        crate::domain::entities::event::EventName::JobStarted => "job_started".to_string(),
        crate::domain::entities::event::EventName::JobSucceeded => "job_succeeded".to_string(),
        crate::domain::entities::event::EventName::JobFailed => "job_failed".to_string(),
        crate::domain::entities::event::EventName::JobCanceled => "job_canceled".to_string(),
    }
}
