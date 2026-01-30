use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct ApiKeyResult {
    pub api_key: Option<String>,
    pub key_id: uuid::Uuid,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub key_prefix: String,
}

#[derive(Debug)]
pub enum ApiKeyUseCaseError {
    NotFound,
    Storage(String),
}
