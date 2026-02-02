use crate::infrastructure::db::dto::ReportRow;
use crate::infrastructure::db::postgres::PostgresDatabase;
use crate::infrastructure::db::stores::report_store::{ReportRepositoryError, ReportStore};
use async_trait::async_trait;
use sqlx::PgConnection;

#[derive(Clone)]
pub struct ReportStorePostgres {
    db: std::sync::Arc<PostgresDatabase>,
}

impl ReportStorePostgres {
    /// Build a Postgres-backed report store.
    pub fn new(db: std::sync::Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    async fn get_impl_conn(
        conn: &mut PgConnection,
        job_id: uuid::Uuid,
    ) -> Result<Option<ReportRow>, ReportRepositoryError> {
        let row = sqlx::query_as::<_, ReportRow>(
            "SELECT
                job_id,
                outcome,
                outcome_reason,
                started_at,
                finished_at,
                duration_ms,
                created_at
            FROM reports
            WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(row)
    }

    async fn insert_impl_conn(
        conn: &mut PgConnection,
        row: &ReportRow,
    ) -> Result<ReportRow, ReportRepositoryError> {
        let stored = sqlx::query_as::<_, ReportRow>(
            "INSERT INTO reports (
                job_id,
                outcome,
                outcome_reason,
                started_at,
                finished_at,
                duration_ms,
                created_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            ON CONFLICT (job_id) DO UPDATE SET
                outcome = EXCLUDED.outcome,
                outcome_reason = EXCLUDED.outcome_reason,
                started_at = EXCLUDED.started_at,
                finished_at = EXCLUDED.finished_at,
                duration_ms = EXCLUDED.duration_ms,
                created_at = EXCLUDED.created_at
            RETURNING
                job_id,
                outcome,
                outcome_reason,
                started_at,
                finished_at,
                duration_ms,
                created_at",
        )
        .bind(row.job_id)
        .bind(&row.outcome)
        .bind(&row.outcome_reason)
        .bind(row.started_at)
        .bind(row.finished_at)
        .bind(row.duration_ms)
        .bind(row.created_at)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(stored)
    }

    async fn update_impl_conn(
        conn: &mut PgConnection,
        row: &ReportRow,
    ) -> Result<ReportRow, ReportRepositoryError> {
        let stored = sqlx::query_as::<_, ReportRow>(
            "UPDATE reports SET
                outcome = $2,
                outcome_reason = $3,
                started_at = $4,
                finished_at = $5,
                duration_ms = $6,
                created_at = $7
            WHERE job_id = $1
            RETURNING
                job_id,
                outcome,
                outcome_reason,
                started_at,
                finished_at,
                duration_ms,
                created_at",
        )
        .bind(row.job_id)
        .bind(&row.outcome)
        .bind(&row.outcome_reason)
        .bind(row.started_at)
        .bind(row.finished_at)
        .bind(row.duration_ms)
        .bind(row.created_at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        match stored {
            Some(row) => Ok(row),
            None => Err(ReportRepositoryError::NotFound),
        }
    }

    async fn delete_impl_conn(
        conn: &mut PgConnection,
        job_id: uuid::Uuid,
    ) -> Result<(), ReportRepositoryError> {
        let result = sqlx::query("DELETE FROM reports WHERE job_id = $1")
            .bind(job_id)
            .execute(&mut *conn)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        if result.rows_affected() == 0 {
            return Err(ReportRepositoryError::NotFound);
        }

        Ok(())
    }
}

#[async_trait]
impl ReportStore for ReportStorePostgres {
    /// Fetch a report by job ID. Returns `None` if it doesn't exist.
    async fn get(&self, job_id: uuid::Uuid) -> Result<Option<ReportRow>, ReportRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::get_impl_conn(conn, job_id)))
            .await
    }

    /// Create a report and return exactly what was stored in the database.
    async fn insert(&self, row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::insert_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Update a report and return exactly what was stored in the database.
    async fn update(&self, row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
        let row = row.clone();
        self.db
            .with_conn(move |conn| {
                let row = row;
                Box::pin(async move { Self::update_impl_conn(conn, &row).await })
            })
            .await
    }

    /// Delete a report by job ID. Returns an error if it doesn't exist.
    async fn delete(&self, job_id: uuid::Uuid) -> Result<(), ReportRepositoryError> {
        self.db
            .with_conn(move |conn| Box::pin(Self::delete_impl_conn(conn, job_id)))
            .await
    }

    /// Create a report inside an existing transaction and return the stored row.
    async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ReportRow,
    ) -> Result<ReportRow, ReportRepositoryError> {
        Self::insert_impl_conn(&mut *tx, row).await
    }

    /// Update a report inside an existing transaction and return the stored row.
    async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &ReportRow,
    ) -> Result<ReportRow, ReportRepositoryError> {
        Self::update_impl_conn(&mut *tx, row).await
    }

    /// Delete a report inside an existing transaction. Returns an error if it doesn't exist.
    async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<(), ReportRepositoryError> {
        Self::delete_impl_conn(&mut *tx, job_id).await
    }

    /// Fetch a report by job ID inside an existing transaction.
    async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: uuid::Uuid,
    ) -> Result<Option<ReportRow>, ReportRepositoryError> {
        Self::get_impl_conn(&mut *tx, job_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::ReportStorePostgres;
    use crate::domain::entities::job::Job;
    use crate::domain::value_objects::ids::{ClientId, JobId};
    use crate::infrastructure::db::dto::{JobRow, ReportRow};
    use crate::infrastructure::db::postgres::PostgresDatabase;
    use crate::infrastructure::db::postgres::job_store_postgres::JobStorePostgres;
    use crate::infrastructure::db::stores::job_store::JobStore;
    use crate::infrastructure::db::stores::report_store::{ReportRepositoryError, ReportStore};
    use time::OffsetDateTime;

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    async fn setup_store() -> Option<ReportStorePostgres> {
        let url = test_db_url()?;
        let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.ok()?);
        Some(ReportStorePostgres::new(db))
    }

    async fn create_job_id() -> Option<JobId> {
        let url = test_db_url()?;
        let db = std::sync::Arc::new(PostgresDatabase::connect(&url).await.ok()?);
        let job_store = JobStorePostgres::new(db);
        let job = Job::new_instant(
            JobId::new(),
            ClientId::new(),
            None,
            None,
            Some("SUCCESS_FAST".to_string()),
        )
        .unwrap();
        let row = JobRow::from_job(&job);
        let stored = job_store.insert(&row).await.ok()?;
        Some(JobId(stored.id))
    }

    fn sample_report_row(job_id: JobId) -> ReportRow {
        let now = OffsetDateTime::now_utc();
        ReportRow {
            job_id: job_id.0,
            outcome: "success".to_string(),
            outcome_reason: None,
            started_at: now,
            finished_at: now,
            duration_ms: 0,
            created_at: now,
        }
    }

    #[tokio::test]
    async fn given_new_report_when_insert_should_return_stored_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_report_row(job_id);

        let stored = store.insert(&row).await.unwrap();

        assert_eq!(stored.job_id, row.job_id);
        assert_eq!(stored.outcome, row.outcome);
    }

    #[tokio::test]
    async fn given_existing_report_when_get_should_return_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_report_row(job_id);
        let stored = store.insert(&row).await.unwrap();

        let fetched = store.get(stored.job_id).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().job_id, stored.job_id);
    }

    #[tokio::test]
    async fn given_missing_report_when_get_should_return_none() {
        let Some(store) = setup_store().await else {
            return;
        };

        let fetched = store.get(uuid::Uuid::new_v4()).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_existing_report_when_update_should_return_stored_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let mut row = sample_report_row(job_id);
        let stored = store.insert(&row).await.unwrap();
        row.outcome = "failed".to_string();
        row.outcome_reason = Some("boom".to_string());
        row.created_at = OffsetDateTime::now_utc();

        let updated = store.update(&row).await.unwrap();

        assert_eq!(updated.job_id, stored.job_id);
        assert_eq!(updated.outcome, "failed");
    }

    #[tokio::test]
    async fn given_missing_report_when_update_should_return_not_found() {
        let Some(store) = setup_store().await else {
            return;
        };
        let row = sample_report_row(JobId::new());

        let err = store.update(&row).await.unwrap_err();

        assert_eq!(err, ReportRepositoryError::NotFound);
    }

    #[tokio::test]
    async fn given_existing_report_when_delete_should_remove_row() {
        let Some(store) = setup_store().await else {
            return;
        };
        let Some(job_id) = create_job_id().await else {
            return;
        };
        let row = sample_report_row(job_id);
        let stored = store.insert(&row).await.unwrap();

        store.delete(stored.job_id).await.unwrap();

        let fetched = store.get(stored.job_id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_missing_report_when_delete_should_return_not_found() {
        let Some(store) = setup_store().await else {
            return;
        };

        let err = store.delete(uuid::Uuid::new_v4()).await.unwrap_err();

        assert_eq!(err, ReportRepositoryError::NotFound);
    }
}
