use crate::infrastructure::db::dto::ApiKeyRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::api_key_store::{ApiKeyRepositoryError, ApiKeyStore};
use async_trait::async_trait;
use sqlx::PgConnection;

#[derive(Clone)]
pub struct ApiKeyStorePostgres {
    db: std::sync::Arc<PostgresDatabase>,
}

impl ApiKeyStorePostgres {
    /// Build a Postgres-backed API key store.
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        key_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        let row = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT
                id,
                client_id,
                key_hash,
                key_prefix,
                created_at,
                expires_at,
                revoked_at
            FROM api_keys
            WHERE id = $1",
        )
        .bind(key_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &ApiKeyRow,
    ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        let stored = sqlx::query_as::<_, ApiKeyRow>(
            "INSERT INTO api_keys (
                id,
                client_id,
                key_hash,
                key_prefix,
                created_at,
                expires_at,
                revoked_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            ON CONFLICT (id) DO UPDATE SET
                id = EXCLUDED.id
            RETURNING
                id,
                client_id,
                key_hash,
                key_prefix,
                created_at,
                expires_at,
                revoked_at",
        )
        .bind(row.id)
        .bind(row.client_id)
        .bind(&row.key_hash)
        .bind(&row.key_prefix)
        .bind(row.created_at)
        .bind(row.expires_at)
        .bind(row.revoked_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)?;

        Ok(stored)
    }

    async fn update_impl_conn(
        conn: &mut PgConnection,
        row: &ApiKeyRow,
    ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        let stored = sqlx::query_as::<_, ApiKeyRow>(
            "UPDATE api_keys SET
                client_id = $2,
                key_hash = $3,
                key_prefix = $4,
                created_at = $5,
                expires_at = $6,
                revoked_at = $7
            WHERE id = $1
            RETURNING
                id,
                client_id,
                key_hash,
                key_prefix,
                created_at,
                expires_at,
                revoked_at",
        )
        .bind(row.id)
        .bind(row.client_id)
        .bind(&row.key_hash)
        .bind(&row.key_prefix)
        .bind(row.created_at)
        .bind(row.expires_at)
        .bind(row.revoked_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)?;

        match stored {
            Some(row) => Ok(row),
            None => Err(ApiKeyRepositoryError::NotFound),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        key_id: uuid::Uuid,
    ) -> Result<(), ApiKeyRepositoryError> {
        let result = sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(key_id)
            .execute(&mut *conn)
            .await
            .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)?;

        if result.rows_affected() == 0 {
            return Err(ApiKeyRepositoryError::NotFound);
        }

        Ok(())
    }

    async fn get_active_by_client_id_impl_conn(
        conn: &mut PgConnection,
        client_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        let row = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT
                id,
                client_id,
                key_hash,
                key_prefix,
                created_at,
                expires_at,
                revoked_at
            FROM api_keys
            WHERE client_id = $1 AND revoked_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1",
        )
        .bind(client_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)?;

        Ok(row)
    }
}

#[async_trait]
impl ApiKeyStore for ApiKeyStorePostgres {
    /// Fetch an API key by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, key_id: uuid::Uuid) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, key_id)))
            .await
    }

    /// Create an API key and return exactly what was stored in the database.
    async fn insert(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Update an API key and return exactly what was stored in the database.
    async fn update(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::update_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Delete an API key by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, key_id: uuid::Uuid) -> Result<(), ApiKeyRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, key_id)))
            .await
    }

    /// Fetch the active (non-revoked) API key for a client.
    async fn get_active_by_client_id(
        &self,
        client_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        self.db
            .with_conn(move |conn| {
                Box::pin(Self::get_active_by_client_id_impl_conn(conn, client_id))
            })
            .await
    }

    /// Create an API key inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ApiKeyRow,
    ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        Self::insert_impl_conn(&mut *tx, row).await
    }

    /// Update an API key inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ApiKeyRow,
    ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        Self::update_impl_conn(&mut *tx, row).await
    }

    /// Delete an API key inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        key_id: uuid::Uuid,
    ) -> Result<(), ApiKeyRepositoryError> {
        Self::delete_impl_conn(&mut *tx, key_id).await
    }

    /// Fetch an API key by ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        key_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        Self::get_impl_conn(&mut *tx, key_id).await
    }

    /// Fetch the active (non-revoked) API key for a client inside an existing transaction.
    async fn get_active_by_client_id_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        client_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        Self::get_active_by_client_id_impl_conn(&mut *tx, client_id).await
    }
}
