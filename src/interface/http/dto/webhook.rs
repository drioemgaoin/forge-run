use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterWebhookRequest {
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterWebhookResponse {
    pub webhook_id: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UnregisterWebhookResponse {
    pub deleted: bool,
}
