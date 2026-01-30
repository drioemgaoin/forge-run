// HTTP routes: client creation.

use crate::application::usecases::create_client::CreateClientUseCase;
use crate::infrastructure::db::postgres::client_store_postgres::ClientStorePostgres;
use crate::interface::http::state::AppState;
use axum::{Json, Router, routing::post};

/// Builds the client routes (currently only client creation).
pub fn router() -> Router<AppState> {
    Router::new().route("/clients", post(create_client))
}

/// Creates a new client and returns its identifier.
///
/// This is a public endpoint used during onboarding.
async fn create_client(state: axum::extract::State<AppState>) -> Json<serde_json::Value> {
    // Step 1: build the store and use case.
    let store = ClientStorePostgres::new(state.db.clone());
    let usecase = CreateClientUseCase { store };
    // Step 3: run the use case and map the output to a JSON payload.
    match usecase.execute().await {
        Ok(client) => Json(serde_json::json!({ "client_id": client.id.0.to_string() })),
        Err(_) => Json(serde_json::json!({ "error": "storage unavailable" })),
    }
}
