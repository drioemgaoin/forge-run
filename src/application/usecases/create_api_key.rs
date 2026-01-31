// Use case: create_api_key.

use crate::application::context::AppContext;
use crate::application::shared::api_key_helpers::generate_api_key;
use crate::application::shared::api_key_types::{ApiKeyResult, ApiKeyUseCaseError};
use crate::domain::value_objects::ids::ClientId;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ApiKeyRow;
use time::OffsetDateTime;

/// Creates API keys for clients (with optional rotation).
pub struct CreateApiKeyUseCase;

/// Input for creating or rotating an API key.
#[derive(Debug, Clone)]
pub struct CreateApiKeyCommand {
    pub client_id: ClientId,
    pub rotate: bool,
}

impl CreateApiKeyUseCase {
    /// Create an API key (or return an existing active one when rotate=false).
    pub async fn execute(
        ctx: &AppContext,
        cmd: CreateApiKeyCommand,
    ) -> Result<ApiKeyResult, ApiKeyUseCaseError> {
        let repo = ctx.repos.api_key.clone();
        let client_id = cmd.client_id.0;
        let rotate = cmd.rotate;
        let now = OffsetDateTime::now_utc();
        // Step 1: Run the flow in a single transaction.
        ctx.repos
            .with_tx(|tx| {
                let repo = repo.clone();
                Box::pin(async move {
                    // Step 2: Check for an existing active key.
                    if let Some(existing) = repo
                        .get_active_by_client_id_tx(tx, client_id)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                    {
                        if !rotate {
                            // Step 3: Return existing key metadata when rotate is false.
                            return Ok(ApiKeyResult {
                                api_key: None,
                                key_id: existing.id,
                                created_at: existing.created_at,
                                expires_at: existing.expires_at,
                                key_prefix: existing.key_prefix,
                            });
                        }
                        // Step 4: Revoke the existing key before creating a new one.
                        let mut revoked = existing.clone();
                        revoked.revoked_at = Some(now);
                        repo.update_tx(tx, &revoked)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    }

                    // Step 5: Create a new key and persist it.
                    let (raw_key, key_prefix, key_hash) = generate_api_key();
                    let row = ApiKeyRow {
                        id: uuid::Uuid::new_v4(),
                        client_id,
                        key_hash,
                        key_prefix,
                        created_at: now,
                        expires_at: None,
                        revoked_at: None,
                    };
                    let stored = repo
                        .insert_tx(tx, &row)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                    // Step 6: Return the new key and metadata.
                    Ok(ApiKeyResult {
                        api_key: Some(raw_key),
                        key_id: stored.id,
                        created_at: stored.created_at,
                        expires_at: stored.expires_at,
                        key_prefix: stored.key_prefix,
                    })
                })
            })
            .await
            .map_err(|e| ApiKeyUseCaseError::Storage(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{CreateApiKeyCommand, CreateApiKeyUseCase};
    use crate::application::context::test_support::test_context;
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::ClientId;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::repositories::Repositories;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_no_active_key_when_execute_should_create_new_key() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db);
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();
        let result = CreateApiKeyUseCase::execute(
            &ctx,
            CreateApiKeyCommand {
                client_id,
                rotate: false,
            },
        )
        .await
        .unwrap();

        assert!(result.api_key.is_some());

        ctx.repos.api_key.delete(result.key_id).await.unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }

    #[tokio::test]
    async fn given_existing_key_when_rotate_false_should_return_existing_metadata() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db);
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();
        let first = CreateApiKeyUseCase::execute(
            &ctx,
            CreateApiKeyCommand {
                client_id,
                rotate: false,
            },
        )
        .await
        .unwrap();

        let second = CreateApiKeyUseCase::execute(
            &ctx,
            CreateApiKeyCommand {
                client_id,
                rotate: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(first.key_id, second.key_id);
        assert!(second.api_key.is_none());

        ctx.repos.api_key.delete(first.key_id).await.unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }
}
