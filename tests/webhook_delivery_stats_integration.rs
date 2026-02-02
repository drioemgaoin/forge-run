use forge_run::domain::entities::event::Event;
use forge_run::domain::entities::job::Job;
use forge_run::domain::value_objects::ids::{ClientId, EventId, JobId};
use forge_run::domain::value_objects::timestamps::Timestamp;
use forge_run::infrastructure::db::dto::WebhookDeliveryRow;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use time::OffsetDateTime;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn given_mixed_deliveries_when_stats_called_should_return_counts() {
    let Some(url) = test_db_url() else {
        return;
    };
    let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.unwrap());
    let repos = Repositories::postgres(db.clone());

    let client_id = ClientId::new();
    repos
        .client
        .insert(&forge_run::domain::entities::client::Client::new(client_id))
        .await
        .unwrap();

    let job = Job::new_instant(
        JobId::new(),
        client_id,
        None,
        None,
        Some("SUCCESS_FAST".to_string()),
    )
    .unwrap();
    let stored_job = repos.job.insert(&job).await.unwrap();

    let event = Event::new_created(EventId::new(), stored_job.id, Timestamp::now_utc());
    let stored_event = repos.event.insert(&event).await.unwrap();

    let now = OffsetDateTime::now_utc();
    let deliveries = vec![
        WebhookDeliveryRow {
            id: uuid::Uuid::new_v4(),
            webhook_id: None,
            target_url: "https://example.com/pending".to_string(),
            event_id: stored_event.id.0,
            job_id: stored_job.id.0,
            event_name: stored_event.event_name.as_str().to_string(),
            attempt: 0,
            status: "pending".to_string(),
            last_error: None,
            response_status: None,
            next_attempt_at: Some(now),
            created_at: now,
            updated_at: now,
            delivered_at: None,
        },
        WebhookDeliveryRow {
            id: uuid::Uuid::new_v4(),
            webhook_id: None,
            target_url: "https://example.com/delivered".to_string(),
            event_id: stored_event.id.0,
            job_id: stored_job.id.0,
            event_name: stored_event.event_name.as_str().to_string(),
            attempt: 1,
            status: "delivered".to_string(),
            last_error: None,
            response_status: Some(200),
            next_attempt_at: None,
            created_at: now,
            updated_at: now,
            delivered_at: Some(now),
        },
        WebhookDeliveryRow {
            id: uuid::Uuid::new_v4(),
            webhook_id: None,
            target_url: "https://example.com/failed".to_string(),
            event_id: stored_event.id.0,
            job_id: stored_job.id.0,
            event_name: stored_event.event_name.as_str().to_string(),
            attempt: 3,
            status: "failed".to_string(),
            last_error: Some("boom".to_string()),
            response_status: None,
            next_attempt_at: None,
            created_at: now,
            updated_at: now,
            delivered_at: None,
        },
    ];

    for delivery in &deliveries {
        repos.webhook_delivery.insert(delivery).await.unwrap();
    }

    let stats = repos.webhook_delivery.stats().await.unwrap();

    assert_eq!(stats.pending, 1);
    assert_eq!(stats.delivered, 1);
    assert_eq!(stats.failed, 1);
}
