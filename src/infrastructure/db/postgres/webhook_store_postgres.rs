use crate::infrastructure::db::dto::WebhookRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};
use async_trait::async_trait;
use sqlx::PgConnection;

#[derive(Clone)]
pub struct WebhookStorePostgres {
    db: std::sync::Arc<PostgresDatabase>,
}

impl WebhookStorePostgres {
    /// Build a Postgres-backed webhook store.
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        webhook_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        let row = sqlx::query_as::<_, WebhookRow>(
            "SELECT
                id,
                client_id,
                url,
                events,
                is_default,
                created_at
            FROM webhooks
            WHERE id = $1",
        )
        .bind(webhook_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| WebhookRepositoryError::StorageUnavailable)?;

        Ok(row)
    }

    async fn get_default_impl_conn(
        conn: &mut PgConnection,
        client_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        let row = sqlx::query_as::<_, WebhookRow>(
            "SELECT
                id,
                client_id,
                url,
                events,
                is_default,
                created_at
            FROM webhooks
            WHERE client_id = $1
              AND is_default = true
            LIMIT 1",
        )
        .bind(client_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| WebhookRepositoryError::StorageUnavailable)?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &WebhookRow,
    ) -> Result<WebhookRow, WebhookRepositoryError> {
        let stored = sqlx::query_as::<_, WebhookRow>(
            "INSERT INTO webhooks (
                id,
                client_id,
                url,
                events,
                is_default,
                created_at
            )
            VALUES ($1,$2,$3,$4,$5,$6)
            ON CONFLICT DO NOTHING
            RETURNING
                id,
                client_id,
                url,
                events,
                is_default,
                created_at",
        )
        .bind(row.id)
        .bind(row.client_id)
        .bind(&row.url)
        .bind(&row.events)
        .bind(row.is_default)
        .bind(row.created_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| WebhookRepositoryError::StorageUnavailable)?;

        match stored {
            Some(row) => Ok(row),
            None => Err(WebhookRepositoryError::Conflict),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        webhook_id: uuid::Uuid,
    ) -> Result<(), WebhookRepositoryError> {
        let result = sqlx::query("DELETE FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .execute(&mut *conn)
            .await
            .map_err(|_| WebhookRepositoryError::StorageUnavailable)?;

        if result.rows_affected() == 0 {
            return Err(WebhookRepositoryError::NotFound);
        }

        Ok(())
    }
}

#[async_trait]
impl WebhookStore for WebhookStorePostgres {
    async fn get(
        &self,
        webhook_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, webhook_id)))
            .await
    }

    async fn get_default_for_client(
        &self,
        client_id: uuid::Uuid,
    ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_default_impl_conn(conn, client_id)))
            .await
    }

    async fn insert(&self, row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    async fn delete(&self, webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, webhook_id)))
            .await
    }
}
