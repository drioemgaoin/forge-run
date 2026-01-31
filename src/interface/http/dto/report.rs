use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReportEventResponse {
    pub event_name: String,
    pub prev_state: String,
    pub next_state: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub job_id: String,
    pub outcome: String,
    pub events: Vec<ReportEventResponse>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
}
