use crate::domain::value_objects::ids::ClientId;
use crate::domain::value_objects::timestamps::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub id: ClientId,
    pub created_at: Timestamp,
}

impl Client {
    pub fn new(id: ClientId) -> Self {
        Self {
            id,
            created_at: Timestamp::now_utc(),
        }
    }
}
