// Use case: register_webhook.

use crate::application::context::AppContext;
use crate::infrastructure::db::dto::WebhookRow;
use time::OffsetDateTime;

/// Registers a webhook that will receive job event callbacks.
pub struct RegisterWebhookUseCase;

#[derive(Debug)]
pub enum RegisterWebhookError {
    Conflict,
    Storage(String),
}

#[derive(Debug, Clone)]
pub struct RegisterWebhookCommand {
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RegisterWebhookResult {
    pub webhook_id: uuid::Uuid,
    pub created_at: OffsetDateTime,
}

impl RegisterWebhookUseCase {
    /// Register a new webhook and return its ID.
    pub async fn execute(
        ctx: &AppContext,
        cmd: RegisterWebhookCommand,
    ) -> Result<RegisterWebhookResult, RegisterWebhookError> {
        // Step 1: Build a webhook row with a new ID.
        let row = WebhookRow {
            id: uuid::Uuid::new_v4(),
            url: cmd.url,
            events: cmd.events,
            created_at: OffsetDateTime::now_utc(),
        };

        // Step 2: Persist the webhook.
        let stored = ctx.repos.webhook.insert(&row).await.map_err(|e| match e {
            crate::infrastructure::db::stores::webhook_store::WebhookRepositoryError::Conflict => {
                RegisterWebhookError::Conflict
            }
            _ => RegisterWebhookError::Storage(format!("{e:?}")),
        })?;

        // Step 3: Return the ID and timestamp.
        Ok(RegisterWebhookResult {
            webhook_id: stored.id,
            created_at: stored.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{RegisterWebhookCommand, RegisterWebhookUseCase};
    use crate::application::context::test_support::test_context;
    use crate::infrastructure::db::dto::WebhookRow;
    use crate::infrastructure::db::stores::webhook_store::{WebhookRepositoryError, WebhookStore};
    use async_trait::async_trait;
    use std::sync::Mutex;
    use time::OffsetDateTime;

    struct DummyStore {
        inserted: Mutex<Option<WebhookRow>>,
    }

    #[async_trait]
    impl WebhookStore for DummyStore {
        async fn get(
            &self,
            _webhook_id: uuid::Uuid,
        ) -> Result<Option<WebhookRow>, WebhookRepositoryError> {
            Ok(None)
        }

        async fn insert(&self, row: &WebhookRow) -> Result<WebhookRow, WebhookRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn delete(&self, _webhook_id: uuid::Uuid) -> Result<(), WebhookRepositoryError> {
            Err(WebhookRepositoryError::InvalidInput)
        }
    }

    #[tokio::test]
    async fn given_valid_request_when_execute_should_register_webhook() {
        let store = DummyStore {
            inserted: Mutex::new(None),
        };
        let mut ctx = test_context();
        ctx.repos.webhook = std::sync::Arc::new(
            crate::infrastructure::db::repositories::webhook_repository::WebhookRepository::new(
                std::sync::Arc::new(store),
            ),
        );

        let result = RegisterWebhookUseCase::execute(
            &ctx,
            RegisterWebhookCommand {
                url: "https://example.com/webhook".to_string(),
                events: vec!["job_created".to_string()],
            },
        )
        .await
        .unwrap();

        assert!(!result.webhook_id.is_nil());
        assert!(result.created_at <= OffsetDateTime::now_utc());
    }
}
