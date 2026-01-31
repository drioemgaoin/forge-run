use forge_run::domain::entities::client::Client;
use forge_run::domain::value_objects::ids::ClientId;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
use forge_run::infrastructure::db::repositories::client_repository::ClientRepository;
use std::sync::Arc;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn setup_repo() -> Option<ClientRepository> {
    let url = test_db_url()?;
    let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.ok()?);
    let store = ClientStorePostgres::new(db);
    Some(ClientRepository::new(Arc::new(store)))
}

fn sample_client() -> Client {
    Client::new(ClientId::new())
}

#[tokio::test]
async fn given_client_when_insert_should_return_stored_client() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let client = sample_client();

    let stored = repo.insert(&client).await.unwrap();

    assert_eq!(stored.id, client.id);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_existing_client_when_get_should_return_client() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let client = sample_client();
    let stored = repo.insert(&client).await.unwrap();

    let fetched = repo.get(stored.id).await.unwrap();

    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().id, stored.id);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_existing_client_when_update_should_return_updated_client() {
    let Some(repo) = setup_repo().await else {
        return;
    };
    let mut client = sample_client();
    let stored = repo.insert(&client).await.unwrap();
    client.id = stored.id;

    let updated = repo.update(&client).await.unwrap();

    assert_eq!(updated.id, stored.id);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_missing_client_when_get_should_return_none() {
    let Some(repo) = setup_repo().await else {
        return;
    };

    let fetched = repo.get(ClientId::new()).await.unwrap();

    assert!(fetched.is_none());
}
