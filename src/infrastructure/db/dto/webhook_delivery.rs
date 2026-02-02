use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WebhookDeliveryRow {
    pub id: uuid::Uuid,
    pub webhook_id: Option<uuid::Uuid>,
    pub target_url: String,
    pub event_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
    pub event_name: String,
    pub attempt: i32,
    pub status: String,
    pub last_error: Option<String>,
    pub response_status: Option<i32>,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub delivered_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct WebhookDeliveryStats {
    pub pending: i64,
    pub delivered: i64,
    pub failed: i64,
}

impl WebhookDeliveryRow {
    pub fn is_pending(&self) -> bool {
        self.status == "pending"
    }

    pub fn is_delivered(&self) -> bool {
        self.status == "delivered"
    }

    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }
}
