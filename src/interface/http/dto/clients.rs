use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CreateClientResponse {
    pub client_id: String,
}
