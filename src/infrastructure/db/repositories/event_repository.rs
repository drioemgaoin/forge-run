use crate::domain::entities::event::Event;
use crate::domain::value_objects::ids::EventId;
use crate::infrastructure::db::dto::EventRow;
use crate::infrastructure::db::stores::event_store::{EventRepositoryError, EventStore};
use std::sync::Arc;

pub struct EventRepository<S: EventStore> {
    store: Arc<S>,
}

impl<S: EventStore> EventRepository<S> {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Create an event and return what was actually stored in the database.
    pub async fn insert(&self, event: &Event) -> Result<Event, EventRepositoryError> {
        // Convert entity to DTO
        let dto = EventRow::from_event(event);

        // Insert and return the stored row from DB
        let stored = self
            .store
            .insert(&dto)
            .await
            .map_err(|_| EventRepositoryError::StorageUnavailable)?;

        // Return the event created
        Ok(stored.into_event())
    }

    /// Fetch an event by its ID. Returns `None` if it doesn't exist.
    pub async fn get(&self, event_id: EventId) -> Result<Option<Event>, EventRepositoryError> {
        if let Some(dto) = self
            .store
            .get(event_id.0)
            .await
            .map_err(|_| EventRepositoryError::StorageUnavailable)?
        {
            Ok(Some(dto.into_event()))
        } else {
            Ok(None)
        }
    }

    /// Update an event and return what was actually stored in the database.
    pub async fn update(&self, event: &Event) -> Result<Event, EventRepositoryError> {
        // Convert entity to DTO
        let dto = EventRow::from_event(event);

        // Update and return the stored row from DB
        let stored = self
            .store
            .update(&dto)
            .await
            .map_err(|_| EventRepositoryError::StorageUnavailable)?;

        // Return the event updated
        Ok(stored.into_event())
    }

    /// Delete an event by its ID. Returns an error if it doesn't exist.
    pub async fn delete(&self, event_id: EventId) -> Result<(), EventRepositoryError> {
        self.store
            .delete(event_id.0)
            .await
            .map_err(|_| EventRepositoryError::StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::EventRepository;
    use crate::domain::entities::event::{Event, EventName};
    use crate::domain::entities::job::JobState;
    use crate::domain::value_objects::ids::{EventId, JobId};
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::EventRow;
    use crate::infrastructure::db::stores::event_store::{EventRepositoryError, EventStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    struct DummyStore {
        pub inserted: Mutex<Option<EventRow>>,
        pub get_result: Mutex<Option<Option<EventRow>>>,
        pub get_by_job_result: Mutex<Option<Option<EventRow>>>,
        pub list_result: Mutex<Vec<EventRow>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
                get_result: Mutex::new(None),
                get_by_job_result: Mutex::new(None),
                list_result: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl EventStore for DummyStore {
        async fn get(
            &self,
            _event_id: uuid::Uuid,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Ok(self.get_result.lock().unwrap().take().unwrap_or(None))
        }

        async fn insert(&self, row: &EventRow) -> Result<EventRow, EventRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn update(&self, _row: &EventRow) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn delete(&self, _event_id: uuid::Uuid) -> Result<(), EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn get_by_job_id_and_name(
            &self,
            _job_id: uuid::Uuid,
            _event_name: &str,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Ok(self
                .get_by_job_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(None))
        }

        async fn list_by_job_id(
            &self,
            _job_id: uuid::Uuid,
        ) -> Result<Vec<EventRow>, EventRepositoryError> {
            Ok(self.list_result.lock().unwrap().clone())
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &EventRow,
        ) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &EventRow,
        ) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _event_id: uuid::Uuid,
        ) -> Result<(), EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _event_id: uuid::Uuid,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn get_by_job_id_and_name_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
            _event_name: &str,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn list_by_job_id_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Vec<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }
    }

    fn sample_event() -> Event {
        Event {
            id: EventId::new(),
            job_id: JobId::new(),
            event_name: EventName::JobCreated,
            prev_state: JobState::Created,
            next_state: JobState::Created,
            timestamp: Timestamp::now_utc(),
        }
    }

    #[tokio::test]
    async fn given_event_when_insert_should_return_stored_event() {
        let store = Arc::new(DummyStore::new());
        let repo = EventRepository::new(store);
        let event = sample_event();

        let stored = repo.insert(&event).await.unwrap();

        assert_eq!(stored.id, event.id);
        assert_eq!(stored.job_id, event.job_id);
        assert_eq!(stored.event_name, event.event_name);
    }

    #[tokio::test]
    async fn given_event_row_when_get_should_return_event() {
        let store = Arc::new(DummyStore::new());
        let repo = EventRepository::new(store.clone());
        let row = EventRow {
            id: uuid::Uuid::new_v4(),
            job_id: uuid::Uuid::new_v4(),
            event_name: "job_created".to_string(),
            prev_state: "created".to_string(),
            next_state: "created".to_string(),
            occurred_at: OffsetDateTime::now_utc(),
        };
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let fetched = repo.get(EventId(row.id)).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id.0, row.id);
    }

    #[tokio::test]
    async fn given_missing_event_when_get_should_return_none() {
        let store = Arc::new(DummyStore::new());
        let repo = EventRepository::new(store.clone());
        *store.get_result.lock().unwrap() = Some(None);

        let fetched = repo.get(EventId::new()).await.unwrap();

        assert!(fetched.is_none());
    }
}
