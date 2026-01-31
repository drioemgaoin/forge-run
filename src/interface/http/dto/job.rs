use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SubmitJobRequest {
    pub r#type: String,
    pub execution_at: Option<String>,
    pub callback: Option<String>,
    pub idempotency_key: Option<String>,
    pub work_kind: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitJobResponse {
    pub job_id: String,
    pub state: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub job_id: String,
    pub r#type: String,
    pub state: String,
    pub outcome: Option<String>,
    pub created_at: String,
    pub execution_at: Option<String>,
    pub updated_at: String,
    pub attempt: i64,
    pub callback: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobStateResponse {
    pub job_id: String,
    pub state: String,
    pub updated_at: String,
    pub attempt: Option<i64>,
}
