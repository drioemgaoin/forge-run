use crate::domain::entities::client::Client;
use crate::domain::value_objects::ids::ClientId;
use crate::domain::value_objects::timestamps::Timestamp;
use time::OffsetDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClientRow {
    pub id: uuid::Uuid,
    pub created_at: OffsetDateTime,
}

impl ClientRow {
    pub fn from_client(client: &Client) -> Self {
        Self {
            id: client.id.0,
            created_at: client.created_at.as_inner(),
        }
    }

    pub fn into_client(self) -> Client {
        Client {
            id: ClientId(self.id),
            created_at: Timestamp::from(self.created_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ClientRow;
    use crate::domain::entities::client::Client;
    use crate::domain::value_objects::ids::ClientId;
    use crate::domain::value_objects::timestamps::Timestamp;
    use time::OffsetDateTime;

    fn sample_client() -> Client {
        Client::new(ClientId::new())
    }

    #[test]
    fn given_client_when_from_client_should_map_fields() {
        let client = sample_client();

        let row = ClientRow::from_client(&client);

        assert_eq!(row.id, client.id.0);
        assert_eq!(row.created_at, client.created_at.as_inner());
    }

    #[test]
    fn given_client_row_when_into_client_should_map_fields() {
        let now = OffsetDateTime::now_utc();
        let row = ClientRow {
            id: uuid::Uuid::new_v4(),
            created_at: now,
        };

        let client = row.clone().into_client();

        assert_eq!(client.id.0, row.id);
        assert_eq!(client.created_at, Timestamp::from(row.created_at));
    }
}
