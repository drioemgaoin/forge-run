use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::EventRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::event_store::{EventRepositoryError, EventStore};
use async_trait::async_trait;
use sqlx::PgConnection;

pub struct EventStorePostgres {
    db: PostgresDatabase,
}

impl EventStorePostgres {
    /// Build a Postgres-backed event store.
    pub fn new(db: PostgresDatabase) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        event_id: uuid::Uuid,
    ) -> Result<Option<EventRow>, EventRepositoryError> {
        let row = sqlx::query_as::<_, EventRow>(
            "SELECT
                id,
                job_id,
                event_name,
                prev_state,
                next_state,
                occurred_at
            FROM events
            WHERE id = $1",
        )
        .bind(event_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => EventRepositoryError::StorageUnavailable,
        })?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &EventRow,
    ) -> Result<EventRow, EventRepositoryError> {
        let stored = sqlx::query_as::<_, EventRow>(
            "INSERT INTO events (
                id,
                job_id,
                event_name,
                prev_state,
                next_state,
                occurred_at
            )
            VALUES ($1,$2,$3,$4,$5,$6)
            ON CONFLICT (id) DO UPDATE SET
                id = EXCLUDED.id
            RETURNING
                id,
                job_id,
                event_name,
                prev_state,
                next_state,
                occurred_at",
        )
        .bind(row.id)
        .bind(row.job_id)
        .bind(&row.event_name)
        .bind(&row.prev_state)
        .bind(&row.next_state)
        .bind(row.occurred_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => EventRepositoryError::StorageUnavailable,
        })?;

        Ok(stored)
    }

    async fn update_impl_conn(
        conn: &mut PgConnection,
        row: &EventRow,
    ) -> Result<EventRow, EventRepositoryError> {
        let stored = sqlx::query_as::<_, EventRow>(
            "UPDATE events SET
                event_name = $2,
                prev_state = $3,
                next_state = $4,
                occurred_at = $5
            WHERE id = $1
            RETURNING
                id,
                job_id,
                event_name,
                prev_state,
                next_state,
                occurred_at",
        )
        .bind(row.id)
        .bind(&row.event_name)
        .bind(&row.prev_state)
        .bind(&row.next_state)
        .bind(row.occurred_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => EventRepositoryError::StorageUnavailable,
        })?;

        match stored {
            Some(row) => Ok(row),
            None => Err(EventRepositoryError::NotFound),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        event_id: uuid::Uuid,
    ) -> Result<(), EventRepositoryError> {
        let result = sqlx::query("DELETE FROM events WHERE id = $1")
            .bind(event_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| match DatabaseError::Query(e.to_string()) {
                _ => EventRepositoryError::StorageUnavailable,
            })?;

        if result.rows_affected() == 0 {
            return Err(EventRepositoryError::NotFound);
        }

        Ok(())
    }
}

#[async_trait]
impl EventStore for EventStorePostgres {
    /// Fetch an event by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, event_id: uuid::Uuid) -> Result<Option<EventRow>, EventRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, event_id)))
            .await
    }

    /// Create an event and return exactly what was stored in the database.
    async fn insert(&self, row: &EventRow) -> Result<EventRow, EventRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Update an event and return exactly what was stored in the database.
    async fn update(&self, row: &EventRow) -> Result<EventRow, EventRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::update_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Delete an event by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, event_id: uuid::Uuid) -> Result<(), EventRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, event_id)))
            .await
    }

    /// Create a job inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &EventRow,
    ) -> Result<EventRow, EventRepositoryError> {
        Self::insert_impl_conn(&mut *tx, row).await
    }

    /// Update a job inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &EventRow,
    ) -> Result<EventRow, EventRepositoryError> {
        Self::update_impl_conn(&mut *tx, row).await
    }

    /// Delete a job inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: uuid::Uuid,
    ) -> Result<(), EventRepositoryError> {
        Self::delete_impl_conn(&mut *tx, event_id).await
    }

    /// Fetch a job by ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event_id: uuid::Uuid,
    ) -> Result<Option<EventRow>, EventRepositoryError> {
        Self::get_impl_conn(&mut *tx, event_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::EventStorePostgres;
    use crate::domain::entities::job::Job;
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::infrastructure::db::dto::EventRow;
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
    use crate::infrastructure::db::stores::event_store::{EventRepositoryError, EventStore};
    use crate::infrastructure::db::stores::job_store::JobStore;
    use time::OffsetDateTime;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    async fn create_job_id() -> Option<JobId> {
        let url = test_db_url()?;
        let db = PostgresDatabase::connect(&url).await.ok()?;
        let job_store = JobStorePostgres::new(db);
        let job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        let row = JobRow::from_job(&job);
        let stored = job_store.insert(&row).await.ok()?;
        Some(JobId(stored.id))
    }

    fn sample_event_row(id: uuid::Uuid, job_id: JobId) -> EventRow {
        EventRow {
            id,
            job_id: job_id.0,
            event_name: "job_created".to_string(),
            prev_state: "created".to_string(),
            next_state: "created".to_string(),
            occurred_at: OffsetDateTime::now_utc(),
        }
    }

    async fn setup_store() -> Option<EventStorePostgres> {
        let url = test_db_url()?;
        let db = PostgresDatabase::connect(&url).await.ok()?;
        Some(EventStorePostgres::new(db))
    }

    #[tokio::test]
    async fn given_new_event_when_insert_should_return_stored_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_event_row(uuid::Uuid::new_v4(), job_id);

        let stored = store.insert(&row).await.unwrap();

        assert_eq!(stored.id, row.id);
        assert_eq!(stored.job_id, row.job_id);
        assert_eq!(stored.event_name, row.event_name);
    }

    #[tokio::test]
    async fn given_existing_event_when_get_should_return_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_event_row(uuid::Uuid::new_v4(), job_id);
        let stored = store.insert(&row).await.unwrap();

        let fetched = store.get(stored.id).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, stored.id);
    }

    #[tokio::test]
    async fn given_missing_event_when_get_should_return_none() {
        let Some(store) = setup_store().await else {
            return;
        };

        let fetched = store.get(uuid::Uuid::new_v4()).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_existing_event_when_update_should_return_stored_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let mut row = sample_event_row(uuid::Uuid::new_v4(), job_id);
        let stored = store.insert(&row).await.unwrap();
        row.event_name = "job_queued".to_string();
        row.prev_state = "created".to_string();
        row.next_state = "queued".to_string();
        row.occurred_at = OffsetDateTime::now_utc();

        let updated = store.update(&row).await.unwrap();

        assert_eq!(updated.id, stored.id);
        assert_eq!(updated.event_name, "job_queued");
        assert_eq!(updated.next_state, "queued");
    }

    #[tokio::test]
    async fn given_missing_event_when_update_should_return_not_found() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_event_row(uuid::Uuid::new_v4(), job_id);

        let err = store.update(&row).await.unwrap_err();

        assert_eq!(err, EventRepositoryError::NotFound);
    }

    #[tokio::test]
    async fn given_existing_event_when_delete_should_remove_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_event_row(uuid::Uuid::new_v4(), job_id);
        let stored = store.insert(&row).await.unwrap();

        store.delete(stored.id).await.unwrap();

        let fetched = store.get(stored.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_missing_event_when_delete_should_return_not_found() {
        let Some(store) = setup_store().await else {
            return;
        };

        let err = store.delete(uuid::Uuid::new_v4()).await.unwrap_err();

        assert_eq!(err, EventRepositoryError::NotFound);
    }
}
