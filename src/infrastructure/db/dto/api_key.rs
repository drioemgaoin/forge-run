use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: uuid::Uuid,
    pub client_id: uuid::Uuid,
    pub key_hash: String,
    pub key_prefix: String,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::ApiKeyRow;
    use time::OffsetDateTime;

    #[test]
    fn given_api_key_row_should_hold_values() {
        let now = OffsetDateTime::now_utc();
        let row = ApiKeyRow {
            id: uuid::Uuid::new_v4(),
            client_id: uuid::Uuid::new_v4(),
            key_hash: "hash".to_string(),
            key_prefix: "pref".to_string(),
            created_at: now,
            expires_at: Some(now),
            revoked_at: None,
        };

        assert_eq!(row.key_hash, "hash");
        assert_eq!(row.key_prefix, "pref");
        assert_eq!(row.expires_at, Some(now));
        assert_eq!(row.revoked_at, None);
    }
}
