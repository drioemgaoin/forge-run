// Use case: create_client.

use crate::domain::entities::client::Client;
use crate::domain::value_objects::ids::ClientId;
use crate::infrastructure::db::dto::ClientRow;
use crate::infrastructure::db::stores::client_store::ClientStore;

/// Creates a new client record.
pub struct CreateClientUseCase<S: ClientStore> {
    pub store: S,
}

#[derive(Debug)]
pub enum CreateClientError {
    Storage(String),
}

impl<S: ClientStore + Send + Sync> CreateClientUseCase<S> {
    /// Create a new client and return it.
    pub async fn execute(&self) -> Result<Client, CreateClientError> {
        // Step 1: Build a new domain client (generates ID and timestamps).
        let client = Client::new(ClientId::new());

        // Step 2: Convert to a database row.
        let row = ClientRow::from_client(&client);

        // Step 3: Persist the row.
        let stored = self
            .store
            .insert(&row)
            .await
            .map_err(|e| CreateClientError::Storage(format!("{e:?}")))?;

        // Step 4: Return the stored client.
        Ok(stored.into_client())
    }
}

#[cfg(test)]
mod tests {
    use super::{CreateClientError, CreateClientUseCase};
    use crate::infrastructure::db::dto::ClientRow;
    use crate::infrastructure::db::stores::client_store::{ClientRepositoryError, ClientStore};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct DummyStore {
        inserted: Mutex<Option<ClientRow>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ClientStore for DummyStore {
        async fn get(
            &self,
            _client_id: uuid::Uuid,
        ) -> Result<Option<ClientRow>, ClientRepositoryError> {
            Ok(None)
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

    #[tokio::test]
    async fn given_valid_request_when_execute_should_create_client() {
        let store = DummyStore::new();
        let usecase = CreateClientUseCase { store };

        let client = usecase.execute().await.unwrap();

        assert!(!client.id.0.is_nil());
    }

    #[tokio::test]
    async fn given_storage_error_when_execute_should_return_error() {
        struct ErrorStore;

        #[async_trait]
        impl ClientStore for ErrorStore {
            async fn get(
                &self,
                _client_id: uuid::Uuid,
            ) -> Result<Option<ClientRow>, ClientRepositoryError> {
                Ok(None)
            }

            async fn insert(&self, _row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
                Err(ClientRepositoryError::StorageUnavailable)
            }

            async fn update(&self, _row: &ClientRow) -> Result<ClientRow, ClientRepositoryError> {
                Err(ClientRepositoryError::InvalidInput)
            }

            async fn delete(&self, _client_id: uuid::Uuid) -> Result<(), ClientRepositoryError> {
                Err(ClientRepositoryError::InvalidInput)
            }
        }

        let usecase = CreateClientUseCase { store: ErrorStore };
        let result = usecase.execute().await;

        assert!(matches!(result, Err(CreateClientError::Storage(_))));
    }
}
