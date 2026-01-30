// Use case: revoke_api_key.

use crate::application::shared::api_key_types::ApiKeyUseCaseError;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::api_key_store::ApiKeyStore;
use std::sync::Arc;
use time::OffsetDateTime;

/// Revokes an API key by ID.
pub struct RevokeApiKeyUseCase<S: ApiKeyStore> {
    db: Arc<PostgresDatabase>,
    store: S,
}

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

impl<S: ApiKeyStore + Send + Sync + Clone + 'static> RevokeApiKeyUseCase<S> {
    pub fn new(db: Arc<PostgresDatabase>, store: S) -> Self {
        Self { db, store }
    }

    /// Revoke an API key by ID.
    pub async fn execute(
        &self,
        cmd: RevokeApiKeyCommand,
    ) -> Result<RevokeApiKeyResult, ApiKeyUseCaseError> {
        let store = self.store.clone();
        let key_id = cmd.key_id;
        let now = OffsetDateTime::now_utc();

        // Step 1: Run the flow in a single transaction.
        self.db
            .with_tx(|tx| {
                let store = store.clone();
                Box::pin(async move {
                    // Step 2: Fetch the key (must exist).
                    let Some(mut existing) = store
                        .get_tx(tx, key_id)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))? else {
                        return Err(DatabaseError::Query("not_found".to_string()));
                    };

                    // Step 3: Mark as revoked if not already.
                    if existing.revoked_at.is_none() {
                        existing.revoked_at = Some(now);
                        store
                            .update_tx(tx, &existing)
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
    use crate::application::usecases::create_api_key::{CreateApiKeyCommand, CreateApiKeyUseCase};
    use crate::domain::value_objects::ids::ClientId;
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::ClientRow;
    use crate::infrastructure::db::postgres::api_key_store_postgres::ApiKeyStorePostgres;
    use crate::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::stores::api_key_store::ApiKeyStore;
    use crate::infrastructure::db::stores::client_store::ClientStore;
    use std::sync::Arc;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_existing_key_when_revoke_should_mark_revoked() {
        let Some(url) = test_db_url() else { return; };
        let db = Arc::new(PostgresDatabase::connect(&url).await.unwrap());
        let api_store = ApiKeyStorePostgres::new(db.clone());
        let client_store = ClientStorePostgres::new(db.clone());

        let client_id = ClientId::new();
        let client_row = ClientRow {
            id: client_id.0,
            created_at: Timestamp::now_utc().as_inner(),
        };
        client_store.insert(&client_row).await.unwrap();

        let create = CreateApiKeyUseCase::new(db.clone(), api_store.clone());
        let created = create
            .execute(CreateApiKeyCommand {
                client_id,
                rotate: false,
            })
            .await
            .unwrap();

        let revoke = RevokeApiKeyUseCase::new(db, api_store.clone());
        let result = revoke
            .execute(RevokeApiKeyCommand {
                key_id: created.key_id,
            })
            .await
            .unwrap();

        assert!(result.revoked);

        let row = api_store.get(created.key_id).await.unwrap().unwrap();
        assert!(row.revoked_at.is_some());

        api_store.delete(created.key_id).await.unwrap();
        client_store.delete(client_id.0).await.unwrap();
    }
}
