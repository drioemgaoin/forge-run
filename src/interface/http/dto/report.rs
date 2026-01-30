use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub job_id: String,
    pub outcome: String,
    pub events: Vec<String>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
}
