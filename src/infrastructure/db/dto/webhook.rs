use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WebhookRow {
    pub id: uuid::Uuid,
    pub client_id: uuid::Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub is_default: bool,
    pub created_at: OffsetDateTime,
}
