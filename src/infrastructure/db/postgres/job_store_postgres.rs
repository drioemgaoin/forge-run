use crate::infrastructure::db::database::DatabaseError;
use crate::infrastructure::db::dto::JobRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::job_store::{JobRepositoryError, JobStore};
use async_trait::async_trait;
use sqlx::PgConnection;
use time::OffsetDateTime;

pub struct JobStorePostgres {
    db: PostgresDatabase,
}

impl JobStorePostgres {
    /// Build a Postgres-backed job store.
    pub fn new(db: PostgresDatabase) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        job_id: uuid::Uuid,
    ) -> Result<Option<JobRow>, JobRepositoryError> {
        let row = sqlx::query_as::<_, JobRow>(
            "SELECT
                id,
                client_id,
                job_type,
                state,
                attempt,
                outcome,
                outcome_reason,
                executed_at,
                created_at,
                updated_at,
                callback_url,
                work_kind
            FROM jobs
            WHERE id = $1",
        )
        .bind(job_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => JobRepositoryError::StorageUnavailable,
        })?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &JobRow,
    ) -> Result<JobRow, JobRepositoryError> {
        let stored = sqlx::query_as::<_, JobRow>(
            "INSERT INTO jobs (
                id,
                client_id,
                job_type,
                state,
                attempt,
                outcome,
                outcome_reason,
                executed_at,
                created_at,
                updated_at,
                callback_url,
                work_kind
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            ON CONFLICT (id) DO UPDATE SET
                id = EXCLUDED.id
            RETURNING
                id,
                client_id,
                job_type,
                state,
                attempt,
                outcome,
                outcome_reason,
                executed_at,
                created_at,
                updated_at,
                callback_url,
                work_kind",
        )
        .bind(row.id)
        .bind(row.client_id)
        .bind(&row.job_type)
        .bind(&row.state)
        .bind(row.attempt)
        .bind(&row.outcome)
        .bind(&row.outcome_reason)
        .bind(row.executed_at)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(&row.callback_url)
        .bind(&row.work_kind)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => JobRepositoryError::StorageUnavailable,
        })?;

        Ok(stored)
    }

    async fn update_impl_conn(
        conn: &mut PgConnection,
        row: &JobRow,
    ) -> Result<JobRow, JobRepositoryError> {
        let stored = sqlx::query_as::<_, JobRow>(
            "UPDATE jobs SET
                state = $2,
                attempt = $3,
                outcome = $4,
                outcome_reason = $5,
                executed_at = $6,
                updated_at = $7
            WHERE id = $1
            RETURNING
                id,
                client_id,
                job_type,
                state,
                attempt,
                outcome,
                outcome_reason,
                executed_at,
                created_at,
                updated_at,
                callback_url,
                work_kind",
        )
        .bind(row.id)
        .bind(&row.state)
        .bind(row.attempt)
        .bind(&row.outcome)
        .bind(&row.outcome_reason)
        .bind(row.executed_at)
        .bind(row.updated_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => JobRepositoryError::StorageUnavailable,
        })?;

        match stored {
            Some(row) => Ok(row),
            None => Err(JobRepositoryError::NotFound),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        job_id: uuid::Uuid,
    ) -> Result<(), JobRepositoryError> {
        let result = sqlx::query("DELETE FROM jobs WHERE id = $1")
            .bind(job_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| match DatabaseError::Query(e.to_string()) {
                _ => JobRepositoryError::StorageUnavailable,
            })?;

        if result.rows_affected() == 0 {
            return Err(JobRepositoryError::NotFound);
        }

        Ok(())
    }

    async fn list_due_deferred_impl_conn(
        conn: &mut PgConnection,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<JobRow>, JobRepositoryError> {
        let rows = sqlx::query_as::<_, JobRow>(
            "SELECT
                id,
                client_id,
                job_type,
                state,
                attempt,
                outcome,
                outcome_reason,
                executed_at,
                created_at,
                updated_at,
                callback_url,
                work_kind
            FROM jobs
            WHERE job_type = 'deferred'
              AND state = 'created'
              AND executed_at IS NOT NULL
              AND executed_at <= $1
            ORDER BY executed_at ASC
            LIMIT $2",
        )
        .bind(now)
        .bind(limit as i64)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => JobRepositoryError::StorageUnavailable,
        })?;

        Ok(rows)
    }

    async fn claim_next_queued_impl_conn(
        conn: &mut PgConnection,
    ) -> Result<Option<JobRow>, JobRepositoryError> {
        let row = sqlx::query_as::<_, JobRow>(
            "WITH next_job AS (
                SELECT id
                FROM jobs
                WHERE state = 'queued'
                ORDER BY created_at ASC
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            UPDATE jobs
            SET state = 'assigned',
                updated_at = NOW()
            WHERE id IN (SELECT id FROM next_job)
            RETURNING
                id,
                client_id,
                job_type,
                state,
                attempt,
                outcome,
                outcome_reason,
                executed_at,
                created_at,
                updated_at,
                callback_url,
                work_kind",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| match DatabaseError::Query(e.to_string()) {
            _ => JobRepositoryError::StorageUnavailable,
        })?;

        Ok(row)
    }
}

#[async_trait]
impl JobStore for JobStorePostgres {
    /// Fetch a job by its ID. Returns `None` if it doesn't exist.
    async fn get(&self, job_id: uuid::Uuid) -> Result<Option<JobRow>, JobRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, job_id)))
            .await
    }

    /// Create a job and return exactly what was stored in the database.
    async fn insert(&self, row: &JobRow) -> Result<JobRow, JobRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Update a job and return exactly what was stored in the database.
    async fn update(&self, row: &JobRow) -> Result<JobRow, JobRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::update_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Delete a job by its ID. Returns an error if it doesn't exist.
    async fn delete(&self, job_id: uuid::Uuid) -> Result<(), JobRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, job_id)))
            .await
    }

    /// List deferred jobs that are due for scheduling.
    async fn list_due_deferred(
        &self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<JobRow>, JobRepositoryError> {
        self.db
            .with_conn(move |conn| {
                Box::pin(Self::list_due_deferred_impl_conn(conn, now, limit))
            })
            .await
    }

    /// Atomically claim the next queued job, if any.
    async fn claim_next_queued(&self) -> Result<Option<JobRow>, JobRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::claim_next_queued_impl_conn(conn)))
            .await
    }

    /// Create a job inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &JobRow,
    ) -> Result<JobRow, JobRepositoryError> {
        Self::insert_impl_conn(&mut *tx, row).await
    }

    /// Update a job inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &JobRow,
    ) -> Result<JobRow, JobRepositoryError> {
        Self::update_impl_conn(&mut *tx, row).await
    }

    /// Delete a job inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<(), JobRepositoryError> {
        Self::delete_impl_conn(&mut *tx, job_id).await
    }

    /// Fetch a job by ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<Option<JobRow>, JobRepositoryError> {
        Self::get_impl_conn(&mut *tx, job_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::JobStorePostgres;
    use crate::infrastructure::db::dto::JobRow;
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::stores::job_store::JobStore;
    use crate::infrastructure::db::stores::job_store::JobRepositoryError;
    use time::OffsetDateTime;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    fn sample_job_row(id: uuid::Uuid, client_id: uuid::Uuid) -> JobRow {
        let now = OffsetDateTime::now_utc();
        JobRow {
            id,
            client_id,
            job_type: "instant".to_string(),
            state: "created".to_string(),
            attempt: 0,
            outcome: None,
            outcome_reason: None,
            executed_at: None,
            created_at: now,
            updated_at: now,
            callback_url: None,
            work_kind: "SUCCESS_FAST".to_string(),
        }
    }

    async fn setup_store() -> Option<JobStorePostgres> {
        let url = test_db_url()?;
        let db = PostgresDatabase::connect(&url).await.ok()?;
        Some(JobStorePostgres::new(db))
    }

    #[tokio::test]
    async fn given_new_job_when_insert_should_return_stored_row() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

        let stored = store.insert(&row).await.unwrap();

        assert_eq!(stored.id, row.id);
        assert_eq!(stored.client_id, row.client_id);
        assert_eq!(stored.job_type, row.job_type);
        assert_eq!(stored.state, row.state);
    }

    #[tokio::test]
    async fn given_existing_job_when_get_should_return_row() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let stored = store.insert(&row).await.unwrap();

        let fetched = store.get(stored.id).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, stored.id);
    }

    #[tokio::test]
    async fn given_missing_job_when_get_should_return_none() {
        let Some(store) = setup_store().await else { return; };

        let fetched = store.get(uuid::Uuid::new_v4()).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_existing_job_when_update_should_return_stored_row() {
        let Some(store) = setup_store().await else { return; };
        let mut row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let stored = store.insert(&row).await.unwrap();
        row.state = "queued".to_string();
        row.attempt = 1;
        row.updated_at = OffsetDateTime::now_utc();

        let updated = store.update(&row).await.unwrap();

        assert_eq!(updated.id, stored.id);
        assert_eq!(updated.state, "queued");
        assert_eq!(updated.attempt, 1);
    }

    #[tokio::test]
    async fn given_missing_job_when_update_should_return_not_found() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

        let err = store.update(&row).await.unwrap_err();

        assert_eq!(err, JobRepositoryError::NotFound);
    }

    #[tokio::test]
    async fn given_existing_job_when_delete_should_remove_row() {
        let Some(store) = setup_store().await else { return; };
        let row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let stored = store.insert(&row).await.unwrap();

        store.delete(stored.id).await.unwrap();

        let fetched = store.get(stored.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_missing_job_when_delete_should_return_not_found() {
        let Some(store) = setup_store().await else { return; };

        let err = store.delete(uuid::Uuid::new_v4()).await.unwrap_err();

        assert_eq!(err, JobRepositoryError::NotFound);
    }

    #[tokio::test]
    async fn given_deferred_job_due_when_list_due_deferred_should_return_row() {
        let Some(store) = setup_store().await else { return; };
        let mut row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        row.job_type = "deferred".to_string();
        row.executed_at = Some(OffsetDateTime::now_utc() - time::Duration::seconds(1));
        let stored = store.insert(&row).await.unwrap();

        let rows = store
            .list_due_deferred(OffsetDateTime::now_utc(), 10)
            .await
            .unwrap();

        assert!(rows.iter().any(|r| r.id == stored.id));
    }

    #[tokio::test]
    async fn given_queued_jobs_when_claim_next_queued_should_assign_one() {
        let Some(store) = setup_store().await else { return; };
        let mut row = sample_job_row(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        row.state = "queued".to_string();
        let stored = store.insert(&row).await.unwrap();

        let claimed = store.claim_next_queued().await.unwrap();

        assert!(claimed.is_some());
        let claimed = claimed.unwrap();
        assert_eq!(claimed.id, stored.id);
        assert_eq!(claimed.state, "assigned");
    }
}
