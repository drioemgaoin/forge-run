use crate::infrastructure::db::dto::{WebhookDeliveryRow, WebhookDeliveryStats};
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::webhook_delivery_store::{
    WebhookDeliveryRepositoryError, WebhookDeliveryStore,
};
use async_trait::async_trait;
use sqlx::PgConnection;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct WebhookDeliveryStorePostgres {
    db: std::sync::Arc<PostgresDatabase>,
}

impl WebhookDeliveryStorePostgres {
    /// Build a Postgres-backed webhook delivery store.
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        delivery_id: uuid::Uuid,
    ) -> Result<Option<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        let row = sqlx::query_as::<_, WebhookDeliveryRow>(
            "SELECT
                id,
                webhook_id,
                target_url,
                event_id,
                job_id,
                event_name,
                attempt,
                status,
                last_error,
                response_status,
                next_attempt_at,
                created_at,
                updated_at,
                delivered_at
            FROM webhook_deliveries
            WHERE id = $1",
        )
        .bind(delivery_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)?;

        Ok(row)
    }

    async fn list_due_impl_conn(
        conn: &mut PgConnection,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        let rows = sqlx::query_as::<_, WebhookDeliveryRow>(
            "SELECT
                id,
                webhook_id,
                target_url,
                event_id,
                job_id,
                event_name,
                attempt,
                status,
                last_error,
                response_status,
                next_attempt_at,
                created_at,
                updated_at,
                delivered_at
            FROM webhook_deliveries
            WHERE status = 'pending'
              AND next_attempt_at IS NOT NULL
              AND next_attempt_at <= $1
            ORDER BY next_attempt_at ASC
            LIMIT $2",
        )
        .bind(now)
        .bind(limit as i64)
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)?;

        Ok(rows)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        let stored = sqlx::query_as::<_, WebhookDeliveryRow>(
            "INSERT INTO webhook_deliveries (
                id,
                webhook_id,
                target_url,
                event_id,
                job_id,
                event_name,
                attempt,
                status,
                last_error,
                response_status,
                next_attempt_at,
                created_at,
                updated_at,
                delivered_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            ON CONFLICT (webhook_id, event_id) DO UPDATE SET
                webhook_id = EXCLUDED.webhook_id
            RETURNING
                id,
                webhook_id,
                target_url,
                event_id,
                job_id,
                event_name,
                attempt,
                status,
                last_error,
                response_status,
                next_attempt_at,
                created_at,
                updated_at,
                delivered_at",
        )
        .bind(row.id)
        .bind(row.webhook_id)
        .bind(&row.target_url)
        .bind(row.event_id)
        .bind(row.job_id)
        .bind(&row.event_name)
        .bind(row.attempt)
        .bind(&row.status)
        .bind(&row.last_error)
        .bind(row.response_status)
        .bind(row.next_attempt_at)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.delivered_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)?;

        Ok(stored)
    }

    async fn update_impl_conn(
        conn: &mut PgConnection,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        let stored = sqlx::query_as::<_, WebhookDeliveryRow>(
            "UPDATE webhook_deliveries SET
                attempt = $2,
                status = $3,
                last_error = $4,
                response_status = $5,
                next_attempt_at = $6,
                updated_at = $7,
                delivered_at = $8
            WHERE id = $1
            RETURNING
                id,
                webhook_id,
                target_url,
                event_id,
                job_id,
                event_name,
                attempt,
                status,
                last_error,
                response_status,
                next_attempt_at,
                created_at,
                updated_at,
                delivered_at",
        )
        .bind(row.id)
        .bind(row.attempt)
        .bind(&row.status)
        .bind(&row.last_error)
        .bind(row.response_status)
        .bind(row.next_attempt_at)
        .bind(row.updated_at)
        .bind(row.delivered_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)?;

        match stored {
            Some(row) => Ok(row),
            None => Err(WebhookDeliveryRepositoryError::NotFound),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        delivery_id: uuid::Uuid,
    ) -> Result<(), WebhookDeliveryRepositoryError> {
        let result = sqlx::query("DELETE FROM webhook_deliveries WHERE id = $1")
            .bind(delivery_id)
            .execute(&mut *conn)
            .await
            .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)?;

        if result.rows_affected() == 0 {
            return Err(WebhookDeliveryRepositoryError::NotFound);
        }

        Ok(())
    }

    async fn stats_impl_conn(
        conn: &mut PgConnection,
    ) -> Result<WebhookDeliveryStats, WebhookDeliveryRepositoryError> {
        let stats = sqlx::query_as::<_, WebhookDeliveryStats>(
            "SELECT
                COALESCE(COUNT(*) FILTER (WHERE status = 'pending'), 0) AS pending,
                COALESCE(COUNT(*) FILTER (WHERE status = 'delivered'), 0) AS delivered,
                COALESCE(COUNT(*) FILTER (WHERE status = 'failed'), 0) AS failed
            FROM webhook_deliveries",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| WebhookDeliveryRepositoryError::StorageUnavailable)?;

        Ok(stats)
    }
}

#[async_trait]
impl WebhookDeliveryStore for WebhookDeliveryStorePostgres {
    async fn get(
        &self,
        delivery_id: uuid::Uuid,
    ) -> Result<Option<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, delivery_id)))
            .await
    }

    async fn list_due(
        &self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRow>, WebhookDeliveryRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::list_due_impl_conn(conn, now, limit)))
            .await
    }

    async fn insert(
        &self,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        let row = row.clone();
        Self::insert_impl_conn(&mut *tx, &row).await
    }

    async fn update(
        &self,
        row: &WebhookDeliveryRow,
    ) -> Result<WebhookDeliveryRow, WebhookDeliveryRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::update_impl_conn(conn, &row).await })
            })
            .await
    }

    async fn delete(&self, delivery_id: uuid::Uuid) -> Result<(), WebhookDeliveryRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, delivery_id)))
            .await
    }

    async fn stats(&self) -> Result<WebhookDeliveryStats, WebhookDeliveryRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::stats_impl_conn(conn)))
            .await
    }
}
