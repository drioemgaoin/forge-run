use crate::infrastructure::db::dto::IdempotencyKeyRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::idempotency_key_store::{
    IdempotencyKeyRepositoryError, IdempotencyKeyStore,
};
use async_trait::async_trait;
use sqlx::PgConnection;

#[derive(Clone)]
pub struct IdempotencyKeyStorePostgres {
    db: std::sync::Arc<PostgresDatabase>,
}

impl IdempotencyKeyStorePostgres {
    /// Build a Postgres-backed idempotency key store.
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
        let row = sqlx::query_as::<_, IdempotencyKeyRow>(
            "SELECT
                client_id,
                idempotency_key,
                job_id,
                created_at
            FROM idempotency_keys
            WHERE client_id = $1 AND idempotency_key = $2",
        )
        .bind(client_id)
        .bind(idempotency_key)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| IdempotencyKeyRepositoryError::StorageUnavailable)?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &IdempotencyKeyRow,
    ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
        let stored = sqlx::query_as::<_, IdempotencyKeyRow>(
            "INSERT INTO idempotency_keys (
                client_id,
                idempotency_key,
                job_id,
                created_at
            )
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (client_id, idempotency_key) DO UPDATE SET
                client_id = EXCLUDED.client_id
            RETURNING
                client_id,
                idempotency_key,
                job_id,
                created_at",
        )
        .bind(row.client_id)
        .bind(&row.idempotency_key)
        .bind(row.job_id)
        .bind(row.created_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| IdempotencyKeyRepositoryError::StorageUnavailable)?;

        Ok(stored)
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<(), IdempotencyKeyRepositoryError> {
        let result = sqlx::query(
            "DELETE FROM idempotency_keys WHERE client_id = $1 AND idempotency_key = $2",
        )
        .bind(client_id)
        .bind(idempotency_key)
        .execute(&mut *conn)
        .await
        .map_err(|_| IdempotencyKeyRepositoryError::StorageUnavailable)?;

        if result.rows_affected() == 0 {
            return Err(IdempotencyKeyRepositoryError::NotFound);
        }

        Ok(())
    }
}

#[async_trait]
impl IdempotencyKeyStore for IdempotencyKeyStorePostgres {
    /// Fetch an idempotency entry by client + key. Returns `None` if it doesn't exist.
    async fn get(
        &self,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
        let key = idempotency_key.to_string();
        self.db
            .with_conn(move |conn| {
                let key = key;
                Box::pin(async move { Self::get_impl_conn(conn, client_id, &key).await })
            })
            .await
    }

    /// Create an idempotency entry and return exactly what was stored in the database.
    async fn insert(
        &self,
        row: &IdempotencyKeyRow,
    ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Delete an idempotency entry. Returns an error if it doesn't exist.
    async fn delete(
        &self,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<(), IdempotencyKeyRepositoryError> {
        let key = idempotency_key.to_string();
        self.db
            .with_conn(move |conn| {
                let key = key;
                Box::pin(async move { Self::delete_impl_conn(conn, client_id, &key).await })
            })
            .await
    }

    /// Create an idempotency entry inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &IdempotencyKeyRow,
    ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
        Self::insert_impl_conn(&mut *tx, row).await
    }

    /// Delete an idempotency entry inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<(), IdempotencyKeyRepositoryError> {
        Self::delete_impl_conn(&mut *tx, client_id, idempotency_key).await
    }

    /// Fetch an idempotency entry by client + key inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
        Self::get_impl_conn(&mut *tx, client_id, idempotency_key).await
    }
}
