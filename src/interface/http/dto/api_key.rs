use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub rotate: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub api_key: String,
    pub key_id: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeApiKeyRequest {
    pub key_id: String,
}

#[derive(Debug, Serialize)]
pub struct RevokeApiKeyResponse {
    pub revoked: bool,
}
