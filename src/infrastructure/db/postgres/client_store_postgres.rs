use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ClientRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::client_store::{ClientRepositoryError, ClientStore};
use async_trait::async_trait;
use sqlx::PgConnection;

#[derive(Clone)]
pub struct ClientStorePostgres {
    db: std::sync::Arc<PostgresDatabase>,
}

impl ClientStorePostgres {
    /// Build a Postgres-backed client store.
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        client_id: uuid::Uuid,
    ) -> Result<Option<ClientRow>, ClientRepositoryError> {
        let row = sqlx::query_as::<_, ClientRow>(
            "SELECT
                id,
                created_at
            FROM clients
            WHERE id = $1",
        )
        .bind(client_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => ClientRepositoryError::StorageUnavailable,
        })?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &ClientRow,
    ) -> Result<ClientRow, ClientRepositoryError> {
        let stored = sqlx::query_as::<_, ClientRow>(
            "INSERT INTO clients (
                id,
                created_at
            )
            VALUES ($1,$2)
            ON CONFLICT (id) DO UPDATE SET
                id = EXCLUDED.id
            RETURNING
                id,
                created_at",
        )
        .bind(row.id)
        .bind(row.created_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => ClientRepositoryError::StorageUnavailable,
        })?;

        Ok(stored)
    }

    async fn update_impl_conn(
        conn: &mut PgConnection,
        row: &ClientRow,
    ) -> Result<ClientRow, ClientRepositoryError> {
        let stored = sqlx::query_as::<_, ClientRow>(
            "UPDATE clients SET
                created_at = $2
            WHERE id = $1
            RETURNING
                id,
                created_at",
        )
        .bind(row.id)
        .bind(row.created_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => ClientRepositoryError::StorageUnavailable,
        })?;

        match stored {
            Some(row) => Ok(row),
            None => Err(ClientRepositoryError::NotFound),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        client_id: uuid::Uuid,
    ) -> Result<(), ClientRepositoryError> {
        let result = sqlx::query("DELETE FROM clients WHERE id = $1")
            .bind(client_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| match DatabaseError::Query(e.to_string()) {
                _ => ClientRepositoryError::StorageUnavailable,
            })?;

        if result.rows_affected() == 0 {
            return Err(ClientRepositoryError::NotFound);
        }

        Ok(())
    }
}

#[async_trait]
impl ClientStore for ClientStorePostgres {
    /// Fetch a client by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, client_id: uuid::Uuid) -> Result<Option<ClientRow>, ClientRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, client_id)))
            .await
    }

    /// Create a client and return exactly what was stored in the database.
    async fn insert(&self, row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Update a client and return exactly what was stored in the database.
    async fn update(&self, row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::update_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Delete a client by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, client_id: uuid::Uuid) -> Result<(), ClientRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, client_id)))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::ClientStorePostgres;
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::ClientId;
    use crate::infrastructure::db::dto::ClientRow;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::stores::client_store::{ClientRepositoryError, ClientStore};

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    async fn setup_store() -> Option<ClientStorePostgres> {
        let url = test_db_url()?;
        let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.ok()?);
        Some(ClientStorePostgres::new(db))
    }

    fn sample_client_row() -> ClientRow {
        let client = Client::new(ClientId::new());
        ClientRow::from_client(&client)
    }

    #[tokio::test]
    async fn given_new_client_when_insert_should_return_stored_row() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_client_row();

        let stored = store.insert(&row).await.unwrap();

        assert_eq!(stored.id, row.id);
    }

    #[tokio::test]
    async fn given_existing_client_when_get_should_return_row() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_client_row();
        let stored = store.insert(&row).await.unwrap();

        let fetched = store.get(stored.id).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, stored.id);
    }

    #[tokio::test]
    async fn given_missing_client_when_get_should_return_none() {
        let Some(store) = setup_store().await else { return; };

        let fetched = store.get(uuid::Uuid::new_v4()).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_existing_client_when_update_should_return_stored_row() {
        let Some(store) = setup_store().await else { return; };
        let mut row = sample_client_row();
        let stored = store.insert(&row).await.unwrap();
        row.id = stored.id;

        let updated = store.update(&row).await.unwrap();

        assert_eq!(updated.id, stored.id);
    }

    #[tokio::test]
    async fn given_missing_client_when_update_should_return_not_found() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_client_row();

        let err = store.update(&row).await.unwrap_err();

        assert_eq!(err, ClientRepositoryError::NotFound);
    }

    #[tokio::test]
    async fn given_existing_client_when_delete_should_remove_row() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_client_row();
        let stored = store.insert(&row).await.unwrap();

        store.delete(stored.id).await.unwrap();

        let fetched = store.get(stored.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_missing_client_when_delete_should_return_not_found() {
        let Some(store) = setup_store().await else { return; };

        let err = store.delete(uuid::Uuid::new_v4()).await.unwrap_err();

        assert_eq!(err, ClientRepositoryError::NotFound);
    }
}
