use forge_run::domain::entities::event::{Event, EventName};
use forge_run::domain::entities::job::JobState;
use forge_run::domain::value_objects::ids::{EventId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::postgres::event_store_postgres::EventStorePostgres;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::event_repository::EventRepository;
use std::sync::Arc;
use time::OffsetDateTime;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn setup_repo() -> Option<EventRepository<EventStorePostgres>> {
    let url = test_db_url()?;
    let db = PostgresDatabase::connect(&url).await.ok()?;
    let store = EventStorePostgres::new(db);
    Some(EventRepository::new(Arc::new(store)))
}

fn sample_event(job_id: JobId) -> Event {
    Event {
        id: EventId::new(),
        job_id,
        event_name: EventName::JobCreated,
        prev_state: JobState::Created,
        next_state: JobState::Created,
        timestamp: Timestamp::now_utc(),
    }
}

#[tokio::test]
async fn given_event_when_insert_should_return_stored_event() {
    let Some(repo) = setup_repo().await else { return; };
    let event = sample_event(JobId::new());

    let stored = repo.insert(&event).await.unwrap();

    assert_eq!(stored.id, event.id);
    assert_eq!(stored.job_id, event.job_id);
    assert_eq!(stored.event_name, event.event_name);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_existing_event_when_get_should_return_event() {
    let Some(repo) = setup_repo().await else { return; };
    let event = sample_event(JobId::new());
    let stored = repo.insert(&event).await.unwrap();

    let fetched = repo.get(stored.id).await.unwrap();

    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().id, stored.id);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_existing_event_when_update_should_return_updated_event() {
    let Some(repo) = setup_repo().await else { return; };
    let mut event = sample_event(JobId::new());
    let stored = repo.insert(&event).await.unwrap();
    event.id = stored.id;
    event.event_name = EventName::JobQueued;
    event.prev_state = JobState::Created;
    event.next_state = JobState::Queued;
    event.timestamp = Timestamp::from(OffsetDateTime::now_utc());

    let updated = repo.update(&event).await.unwrap();

    assert_eq!(updated.id, stored.id);
    assert_eq!(updated.event_name, EventName::JobQueued);
    assert_eq!(updated.next_state, JobState::Queued);
    repo.delete(stored.id).await.unwrap();
}

#[tokio::test]
async fn given_missing_event_when_get_should_return_none() {
    let Some(repo) = setup_repo().await else { return; };

    let fetched = repo.get(EventId::new()).await.unwrap();

    assert!(fetched.is_none());
}
