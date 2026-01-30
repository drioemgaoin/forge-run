// Use case: unregister_webhook.

use crate::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};

/// Removes a previously registered webhook.
pub struct UnregisterWebhookUseCase<S: WebhookStore> {
    pub store: S,
}

#[derive(Debug)]
pub enum UnregisterWebhookError {
    NotFound,
    Storage(String),
}

#[derive(Debug, Clone)]
pub struct UnregisterWebhookCommand {
    pub webhook_id: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct UnregisterWebhookResult {
    pub deleted: bool,
}

impl<S: WebhookStore + Send + Sync> UnregisterWebhookUseCase<S> {
    /// Unregister a webhook by ID.
    pub async fn execute(
        &self,
        cmd: UnregisterWebhookCommand,
    ) -> Result<UnregisterWebhookResult, UnregisterWebhookError> {
        // Step 1: Attempt delete in storage.
        let result = self.store.delete(cmd.webhook_id).await;

        // Step 2: Map storage errors to use case errors.
        match result {
            Ok(()) => Ok(UnregisterWebhookResult { deleted: true }),
            Err(WebhookRepositoryError::NotFound) => Err(UnregisterWebhookError::NotFound),
            Err(err) => Err(UnregisterWebhookError::Storage(format!("{err:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UnregisterWebhookCommand, UnregisterWebhookError, UnregisterWebhookUseCase};
    use crate::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};
    use async_trait::async_trait;

    struct DummyStore {
        result: Result<(), WebhookRepositoryError>,
    }

    #[async_trait]
    impl WebhookStore for DummyStore {
        async fn get(
            &self,
            _webhook_id: uuid::Uuid,
        ) -> Result<Option<crate::infrastructure::db::dto::WebhookRow>, WebhookRepositoryError> {
            Ok(None)
        }

        async fn insert(
            &self,
            _row: &crate::infrastructure::db::dto::WebhookRow,
        ) -> Result<crate::infrastructure::db::dto::WebhookRow, WebhookRepositoryError> {
            Err(WebhookRepositoryError::InvalidInput)
        }

        async fn delete(&self, _webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
            self.result.clone()
        }
    }

    #[tokio::test]
    async fn given_existing_webhook_when_execute_should_delete() {
        let store = DummyStore { result: Ok(()) };
        let usecase = UnregisterWebhookUseCase { store };

        let result = usecase
            .execute(UnregisterWebhookCommand {
                webhook_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();

        assert!(result.deleted);
    }

    #[tokio::test]
    async fn given_missing_webhook_when_execute_should_return_not_found() {
        let store = DummyStore {
            result: Err(WebhookRepositoryError::NotFound),
        };
        let usecase = UnregisterWebhookUseCase { store };

        let result = usecase
            .execute(UnregisterWebhookCommand {
                webhook_id: uuid::Uuid::new_v4(),
            })
            .await;

        assert!(matches!(result, Err(UnregisterWebhookError::NotFound)));
    }
}
