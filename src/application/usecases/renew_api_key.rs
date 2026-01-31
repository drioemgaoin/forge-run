// Use case: renew_api_key.

use crate::application::context::AppContext;
use crate::application::shared::api_key_helpers::generate_api_key;
use crate::application::shared::api_key_types::{ApiKeyResult, ApiKeyUseCaseError};
use crate::domain::value_objects::ids::ClientId;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ApiKeyRow;
use time::OffsetDateTime;

/// Rotates the active API key for a client.
pub struct RenewApiKeyUseCase;

/// Input for key rotation.
#[derive(Debug, Clone)]
pub struct RenewApiKeyCommand {
    pub client_id: ClientId,
}

impl RenewApiKeyUseCase {
    /// Rotate the active API key for a client.
    pub async fn execute(
        ctx: &AppContext,
        cmd: RenewApiKeyCommand,
    ) -> Result<ApiKeyResult, ApiKeyUseCaseError> {
        let repo = ctx.repos.api_key.clone();
        let client_id = cmd.client_id.0;
        let now = OffsetDateTime::now_utc();
        // Step 1: Run the flow in a single transaction.
        ctx.repos
            .with_tx(|tx| {
                let repo = repo.clone();
                Box::pin(async move {
                    // Step 2: Fetch the current active key (must exist).
                    let Some(existing) = repo
                        .get_active_by_client_id_tx(tx, client_id)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                    else {
                        return Err(DatabaseError::Query("not_found".to_string()));
                    };

                    // Step 3: Revoke the existing key.
                    let mut revoked = existing.clone();
                    revoked.revoked_at = Some(now);
                    repo.update_tx(tx, &revoked)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;

                    // Step 4: Create and persist a new key.
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

                    // Step 5: Return the new key and metadata.
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
            .map_err(|e| match e {
                DatabaseError::Query(msg) if msg == "not_found" => ApiKeyUseCaseError::NotFound,
                _ => ApiKeyUseCaseError::Storage(e.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{RenewApiKeyCommand, RenewApiKeyUseCase};
    use crate::application::context::test_support::test_context;
    use crate::application::usecases::create_api_key::{CreateApiKeyCommand, CreateApiKeyUseCase};
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::ClientId;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::repositories::Repositories;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_existing_key_when_renew_should_rotate_and_revoke_old() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());
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

        let second = RenewApiKeyUseCase::execute(&ctx, RenewApiKeyCommand { client_id })
            .await
            .unwrap();

        assert_ne!(first.key_id, second.key_id);

        let old = ctx.repos.api_key.get(first.key_id).await.unwrap().unwrap();
        assert!(old.revoked_at.is_some());

        ctx.repos.api_key.delete(first.key_id).await.unwrap();
        ctx.repos.api_key.delete(second.key_id).await.unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }
}
