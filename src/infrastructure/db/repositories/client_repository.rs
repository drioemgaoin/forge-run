use crate::domain::entities::client::Client;
use crate::domain::value_objects::ids::ClientId;
use crate::infrastructure::db::dto::ClientRow;
use crate::infrastructure::db::stores::client_store::{ClientRepositoryError, ClientStore};
use std::sync::Arc;

pub struct ClientRepository<S: ClientStore> {
    store: Arc<S>,
}

impl<S: ClientStore> ClientRepository<S> {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Create a client and return what was actually stored in the database.
    pub async fn insert(&self, client: &Client) -> Result<Client, ClientRepositoryError> {
        let dto = ClientRow::from_client(client);
        let stored = self
            .store
            .insert(&dto)
            .await
            .map_err(|_| ClientRepositoryError::StorageUnavailable)?;

        Ok(stored.into_client())
    }

    /// Fetch a client by its ID. Returns `None` if it doesn't exist.
    pub async fn get(&self, client_id: ClientId) -> Result<Option<Client>, ClientRepositoryError> {
        if let Some(dto) = self
            .store
            .get(client_id.0)
            .await
            .map_err(|_| ClientRepositoryError::StorageUnavailable)?
        {
            Ok(Some(dto.into_client()))
        } else {
            Ok(None)
        }
    }

    /// Update a client and return what was actually stored in the database.
    pub async fn update(&self, client: &Client) -> Result<Client, ClientRepositoryError> {
        let dto = ClientRow::from_client(client);
        let stored = self
            .store
            .update(&dto)
            .await
            .map_err(|_| ClientRepositoryError::StorageUnavailable)?;

        Ok(stored.into_client())
    }

    /// Delete a client by its ID. Returns an error if it doesn't exist.
    pub async fn delete(&self, client_id: ClientId) -> Result<(), ClientRepositoryError> {
        self.store
            .delete(client_id.0)
            .await
            .map_err(|_| ClientRepositoryError::StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::ClientRepository;
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::ClientId;
    use crate::infrastructure::db::dto::ClientRow;
    use crate::infrastructure::db::stores::client_store::{ClientRepositoryError, ClientStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct DummyStore {
        pub inserted: Mutex<Option<ClientRow>>,
        pub get_result: Mutex<Option<Option<ClientRow>>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
                get_result: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ClientStore for DummyStore {
        async fn get(
            &self,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ClientRow>, ClientRepositoryError> {
            Ok(self.get_result.lock().unwrap().take().unwrap_or(None))
        }

        async fn insert(&self, row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn update(&self, _row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
            Err(ClientRepositoryError::InvalidInput)
        }

        async fn delete(&self, _client_id: uuid::Uuid) -> Result<(), ClientRepositoryError> {
            Err(ClientRepositoryError::InvalidInput)
        }
    }

    fn sample_client() -> Client {
        Client::new(ClientId::new())
    }

    #[tokio::test]
    async fn given_client_when_insert_should_return_stored_client() {
        let store = Arc::new(DummyStore::new());
        let repo = ClientRepository::new(store);
        let client = sample_client();

        let stored = repo.insert(&client).await.unwrap();

        assert_eq!(stored.id, client.id);
    }

    #[tokio::test]
    async fn given_client_row_when_get_should_return_client() {
        let store = Arc::new(DummyStore::new());
        let repo = ClientRepository::new(store.clone());
        let row = ClientRow::from_client(&sample_client());
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let fetched = repo.get(ClientId(row.id)).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id.0, row.id);
    }

    #[tokio::test]
    async fn given_missing_client_when_get_should_return_none() {
        let store = Arc::new(DummyStore::new());
        let repo = ClientRepository::new(store.clone());
        *store.get_result.lock().unwrap() = Some(None);

        let fetched = repo.get(ClientId::new()).await.unwrap();

        assert!(fetched.is_none());
    }
}
