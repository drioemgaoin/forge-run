use crate::infrastructure::db::dto::ApiKeyRow;
use crate::infrastructure::db::stores::api_key_store::{ApiKeyRepositoryError, ApiKeyStore};
use std::sync::Arc;

pub struct ApiKeyRepository<S: ApiKeyStore> {
    store: Arc<S>,
}

impl<S: ApiKeyStore> ApiKeyRepository<S> {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Create an API key and return what was actually stored in the database.
    pub async fn insert(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        self.store
            .insert(row)
            .await
            .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)
    }

    /// Fetch an API key by its ID. Returns `None` if it doesn't exist.
    pub async fn get(
        &self,
        key_id: uuid::Uuid,
    ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
        self.store
            .get(key_id)
            .await
            .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)
    }

    /// Update an API key and return what was actually stored in the database.
    pub async fn update(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
        self.store
            .update(row)
            .await
            .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)
    }

    /// Delete an API key by its ID. Returns an error if it doesn't exist.
    pub async fn delete(&self, key_id: uuid::Uuid) -> Result<(), ApiKeyRepositoryError> {
        self.store
            .delete(key_id)
            .await
            .map_err(|_| ApiKeyRepositoryError::StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::ApiKeyRepository;
    use crate::infrastructure::db::dto::ApiKeyRow;
    use crate::infrastructure::db::stores::api_key_store::{ApiKeyRepositoryError, ApiKeyStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    struct DummyStore {
        pub inserted: Mutex<Option<ApiKeyRow>>,
        pub updated: Mutex<Option<ApiKeyRow>>,
        pub deleted: Mutex<Option<uuid::Uuid>>,
        pub get_result: Mutex<Option<Option<ApiKeyRow>>>,
        pub active_result: Mutex<Option<Option<ApiKeyRow>>>,
        pub auth_result: Mutex<Option<Option<ApiKeyRow>>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
                updated: Mutex::new(None),
                deleted: Mutex::new(None),
                get_result: Mutex::new(None),
                active_result: Mutex::new(None),
                auth_result: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ApiKeyStore for DummyStore {
        async fn get(
            &self,
            key_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            *self.deleted.lock().unwrap() = Some(key_id);
            Ok(self.get_result.lock().unwrap().clone().unwrap_or(None))
        }

        async fn insert(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn update(&self, row: &ApiKeyRow) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            *self.updated.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn delete(&self, key_id: uuid::Uuid) -> Result<(), ApiKeyRepositoryError> {
            *self.deleted.lock().unwrap() = Some(key_id);
            Ok(())
        }

        async fn get_active_by_client_id(
            &self,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Ok(self.active_result.lock().unwrap().clone().unwrap_or(None))
        }

        async fn get_active_by_prefix_and_hash(
            &self,
            _key_prefix: &str,
            _key_hash: &str,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Ok(self.auth_result.lock().unwrap().clone().unwrap_or(None))
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ApiKeyRow,
        ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::InvalidInput)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ApiKeyRow,
        ) -> Result<ApiKeyRow, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::InvalidInput)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _key_id: uuid::Uuid,
        ) -> Result<(), ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::InvalidInput)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _key_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::InvalidInput)
        }

        async fn get_active_by_client_id_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::InvalidInput)
        }

        async fn get_active_by_prefix_and_hash_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _key_prefix: &str,
            _key_hash: &str,
        ) -> Result<Option<ApiKeyRow>, ApiKeyRepositoryError> {
            Err(ApiKeyRepositoryError::InvalidInput)
        }
    }

    fn sample_row() -> ApiKeyRow {
        let now = OffsetDateTime::now_utc();
        ApiKeyRow {
            id: uuid::Uuid::new_v4(),
            client_id: uuid::Uuid::new_v4(),
            key_hash: "hash".to_string(),
            key_prefix: "pref".to_string(),
            created_at: now,
            expires_at: None,
            revoked_at: None,
        }
    }

    #[tokio::test]
    async fn given_row_when_insert_should_call_store_and_return_row() {
        let store = Arc::new(DummyStore::new());
        let repo = ApiKeyRepository::new(store.clone());
        let row = sample_row();

        let stored = repo.insert(&row).await.unwrap();

        assert_eq!(stored.id, row.id);
        assert!(store.inserted.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn given_existing_key_when_get_should_return_row() {
        let store = Arc::new(DummyStore::new());
        let repo = ApiKeyRepository::new(store.clone());
        let row = sample_row();
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let fetched = repo.get(row.id).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, row.id);
    }

    #[tokio::test]
    async fn given_row_when_update_should_call_store_and_return_row() {
        let store = Arc::new(DummyStore::new());
        let repo = ApiKeyRepository::new(store.clone());
        let row = sample_row();

        let updated = repo.update(&row).await.unwrap();

        assert_eq!(updated.id, row.id);
        assert!(store.updated.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn given_key_when_delete_should_call_store() {
        let store = Arc::new(DummyStore::new());
        let repo = ApiKeyRepository::new(store.clone());
        let key_id = uuid::Uuid::new_v4();

        repo.delete(key_id).await.unwrap();

        assert_eq!(store.deleted.lock().unwrap().unwrap(), key_id);
    }
}
