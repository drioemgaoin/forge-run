// Use case: revoke_api_key.

use crate::application::context::AppContext;
use crate::application::shared::api_key_types::ApiKeyUseCaseError;
use crate::infrastructure::db::database::DatabaseError;
use time::OffsetDateTime;

/// Revokes an API key by ID.
pub struct RevokeApiKeyUseCase;

/// Input for key revocation.
#[derive(Debug, Clone)]
pub struct RevokeApiKeyCommand {
    pub key_id: uuid::Uuid,
}

/// Result of a revoke attempt.
#[derive(Debug, Clone)]
pub struct RevokeApiKeyResult {
    pub revoked: bool,
}

impl RevokeApiKeyUseCase {
    /// Revoke an API key by ID.
    pub async fn execute(
        ctx: &AppContext,
        cmd: RevokeApiKeyCommand,
    ) -> Result<RevokeApiKeyResult, ApiKeyUseCaseError> {
        let repo = ctx.repos.api_key.clone();
        let key_id = cmd.key_id;
        let now = OffsetDateTime::now_utc();
        // Step 1: Run the flow in a single transaction.
        ctx.repos
            .with_tx(|tx| {
                let repo = repo.clone();
                Box::pin(async move {
                    // Step 2: Fetch the key (must exist).
                    let Some(mut existing) = repo
                        .get_tx(tx, key_id)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))?
                    else {
                        return Err(DatabaseError::Query("not_found".to_string()));
                    };

                    // Step 3: Mark as revoked if not already.
                    if existing.revoked_at.is_none() {
                        existing.revoked_at = Some(now);
                        repo.update_tx(tx, &existing)
                            .await
                            .map_err(|e| DatabaseError::Query(format!("{e:?}")))?;
                    }

                    // Step 4: Return confirmation.
                    Ok(RevokeApiKeyResult { revoked: true })
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
    use super::{RevokeApiKeyCommand, RevokeApiKeyUseCase};
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
    async fn given_existing_key_when_revoke_should_mark_revoked() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let client_id = ClientId::new();

        let mut ctx = test_context();
        ctx.repos = Repositories::postgres(db.clone());
        let client = Client::new(client_id);
        ctx.repos.client.insert(&client).await.unwrap();
        let created = CreateApiKeyUseCase::execute(
            &ctx,
            CreateApiKeyCommand {
                client_id,
                rotate: false,
            },
        )
        .await
        .unwrap();

        let result = RevokeApiKeyUseCase::execute(
            &ctx,
            RevokeApiKeyCommand {
                key_id: created.key_id,
            },
        )
        .await
        .unwrap();

        assert!(result.revoked);

        let row = ctx
            .repos
            .api_key
            .get(created.key_id)
            .await
            .unwrap()
            .unwrap();
        assert!(row.revoked_at.is_some());

        ctx.repos.api_key.delete(created.key_id).await.unwrap();
        ctx.repos.client.delete(client_id).await.unwrap();
    }
}
