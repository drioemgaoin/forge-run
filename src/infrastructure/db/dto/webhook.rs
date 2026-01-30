use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WebhookRow {
    pub id: uuid::Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub created_at: OffsetDateTime,
}
