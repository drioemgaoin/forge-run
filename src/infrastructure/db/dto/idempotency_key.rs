use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IdempotencyKeyRow {
    pub client_id: uuid::Uuid,
    pub idempotency_key: String,
    pub job_id: Option<uuid::Uuid>,
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::IdempotencyKeyRow;
    use time::OffsetDateTime;

    #[test]
    fn given_idempotency_row_should_hold_values() {
        let now = OffsetDateTime::now_utc();
        let row = IdempotencyKeyRow {
            client_id: uuid::Uuid::new_v4(),
            idempotency_key: "key".to_string(),
            job_id: Some(uuid::Uuid::new_v4()),
            created_at: now,
        };

        assert_eq!(row.idempotency_key, "key");
        assert!(row.job_id.is_some());
        assert_eq!(row.created_at, now);
    }
}
