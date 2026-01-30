use crate::infrastructure::db::dto::IdempotencyKeyRow;
use crate::infrastructure::db::stores::idempotency_key_store::{
    IdempotencyKeyRepositoryError, IdempotencyKeyStore,
};
use std::sync::Arc;

pub struct IdempotencyKeyRepository<S: IdempotencyKeyStore> {
    store: Arc<S>,
}

impl<S: IdempotencyKeyStore> IdempotencyKeyRepository<S> {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Fetch an idempotency entry by client + key. Returns `None` if it doesn't exist.
    pub async fn get(
        &self,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
        self.store
            .get(client_id, idempotency_key)
            .await
            .map_err(|_| IdempotencyKeyRepositoryError::StorageUnavailable)
    }

    /// Create an idempotency entry and return what was actually stored in the database.
    pub async fn insert(
        &self,
        row: &IdempotencyKeyRow,
    ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
        self.store
            .insert(row)
            .await
            .map_err(|_| IdempotencyKeyRepositoryError::StorageUnavailable)
    }

    /// Delete an idempotency entry. Returns an error if it doesn't exist.
    pub async fn delete(
        &self,
        client_id: uuid::Uuid,
        idempotency_key: &str,
    ) -> Result<(), IdempotencyKeyRepositoryError> {
        self.store
            .delete(client_id, idempotency_key)
            .await
            .map_err(|_| IdempotencyKeyRepositoryError::StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::IdempotencyKeyRepository;
    use crate::infrastructure::db::dto::IdempotencyKeyRow;
    use crate::infrastructure::db::stores::idempotency_key_store::{
        IdempotencyKeyRepositoryError, IdempotencyKeyStore,
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    struct DummyStore {
        pub inserted: Mutex<Option<IdempotencyKeyRow>>,
        pub deleted: Mutex<Option<(uuid::Uuid, String)>>,
        pub get_result: Mutex<Option<Option<IdempotencyKeyRow>>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
                deleted: Mutex::new(None),
                get_result: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl IdempotencyKeyStore for DummyStore {
        async fn get(
            &self,
            client_id: uuid::Uuid,
            idempotency_key: &str,
        ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
            *self.deleted.lock().unwrap() =
                Some((client_id, idempotency_key.to_string()));
            Ok(self.get_result.lock().unwrap().clone().unwrap_or(None))
        }

        async fn insert(
            &self,
            row: &IdempotencyKeyRow,
        ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn delete(
            &self,
            client_id: uuid::Uuid,
            idempotency_key: &str,
        ) -> Result<(), IdempotencyKeyRepositoryError> {
            *self.deleted.lock().unwrap() =
                Some((client_id, idempotency_key.to_string()));
            Ok(())
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &IdempotencyKeyRow,
        ) -> Result<IdempotencyKeyRow, IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::InvalidInput)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _client_id: uuid::Uuid,
            _idempotency_key: &str,
        ) -> Result<(), IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::InvalidInput)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _client_id: uuid::Uuid,
            _idempotency_key: &str,
        ) -> Result<Option<IdempotencyKeyRow>, IdempotencyKeyRepositoryError> {
            Err(IdempotencyKeyRepositoryError::InvalidInput)
        }
    }

    fn sample_row() -> IdempotencyKeyRow {
        IdempotencyKeyRow {
            client_id: uuid::Uuid::new_v4(),
            idempotency_key: "key".to_string(),
            job_id: Some(uuid::Uuid::new_v4()),
            created_at: OffsetDateTime::now_utc(),
        }
    }

    #[tokio::test]
    async fn given_row_when_insert_should_call_store_and_return_row() {
        let store = Arc::new(DummyStore::new());
        let repo = IdempotencyKeyRepository::new(store.clone());
        let row = sample_row();

        let stored = repo.insert(&row).await.unwrap();

        assert_eq!(stored.idempotency_key, row.idempotency_key);
        assert!(store.inserted.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn given_existing_key_when_get_should_return_row() {
        let store = Arc::new(DummyStore::new());
        let repo = IdempotencyKeyRepository::new(store.clone());
        let row = sample_row();
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let fetched = repo.get(row.client_id, &row.idempotency_key).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().idempotency_key, row.idempotency_key);
    }

    #[tokio::test]
    async fn given_key_when_delete_should_call_store() {
        let store = Arc::new(DummyStore::new());
        let repo = IdempotencyKeyRepository::new(store.clone());
        let client_id = uuid::Uuid::new_v4();
        let key = "key";

        repo.delete(client_id, key).await.unwrap();

        let deleted = store.deleted.lock().unwrap().clone().unwrap();
        assert_eq!(deleted.0, client_id);
        assert_eq!(deleted.1, key);
    }
}
