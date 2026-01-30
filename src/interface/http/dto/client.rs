use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CreateClientResponse {
    pub client_id: String,
}
