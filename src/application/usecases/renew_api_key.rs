// Use case: renew_api_key.

use crate::application::shared::api_key_helpers::generate_api_key;
use crate::application::shared::api_key_types::{ApiKeyResult, ApiKeyUseCaseError};
use crate::domain::value_objects::ids::ClientId;
use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::ApiKeyRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::api_key_store::ApiKeyStore;
use std::sync::Arc;
use time::OffsetDateTime;

/// Rotates the active API key for a client.
pub struct RenewApiKeyUseCase<S: ApiKeyStore> {
    db: Arc<PostgresDatabase>,
    store: S,
}

/// Input for key rotation.
#[derive(Debug, Clone)]
pub struct RenewApiKeyCommand {
    pub client_id: ClientId,
}

impl<S: ApiKeyStore + Send + Sync + Clone + 'static> RenewApiKeyUseCase<S> {
    pub fn new(db: Arc<PostgresDatabase>, store: S) -> Self {
        Self { db, store }
    }

    /// Rotate the active API key for a client.
    pub async fn execute(
        &self,
        cmd: RenewApiKeyCommand,
    ) -> Result<ApiKeyResult, ApiKeyUseCaseError> {
        let store = self.store.clone();
        let client_id = cmd.client_id.0;
        let now = OffsetDateTime::now_utc();

        // Step 1: Run the flow in a single transaction.
        self.db
            .with_tx(|tx| {
                let store = store.clone();
                Box::pin(async move {
                    // Step 2: Fetch the current active key (must exist).
                    let Some(existing) = store
                        .get_active_by_client_id_tx(tx, client_id)
                        .await
                        .map_err(|e| DatabaseError::Query(format!("{e:?}")))? else {
                        return Err(DatabaseError::Query("not_found".to_string()));
                    };

                    // Step 3: Revoke the existing key.
                    let mut revoked = existing.clone();
                    revoked.revoked_at = Some(now);
                    store
                        .update_tx(tx, &revoked)
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
                    let stored = store
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
    async fn given_existing_key_when_renew_should_rotate_and_revoke_old() {
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
        let first = create
            .execute(CreateApiKeyCommand {
                client_id,
                rotate: false,
            })
            .await
            .unwrap();

        let renew = RenewApiKeyUseCase::new(db, api_store.clone());
        let second = renew
            .execute(RenewApiKeyCommand { client_id })
            .await
            .unwrap();

        assert_ne!(first.key_id, second.key_id);

        let old = api_store.get(first.key_id).await.unwrap().unwrap();
        assert!(old.revoked_at.is_some());

        api_store.delete(first.key_id).await.unwrap();
        api_store.delete(second.key_id).await.unwrap();
        client_store.delete(client_id.0).await.unwrap();
    }
}
