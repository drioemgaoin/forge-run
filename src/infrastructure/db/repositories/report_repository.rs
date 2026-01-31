use crate::domain::entities::report::{EventSnapshot, Report};
use crate::domain::value_objects::ids::JobId;
use crate::infrastructure::db::dto::ReportRow;
use crate::infrastructure::db::stores::report_store::{ReportRepositoryError, ReportStore};
use std::sync::Arc;

pub struct ReportRepository {
    store: Arc<dyn ReportStore>,
}

impl ReportRepository {
    /// Build a repository that uses the given store implementation.
    pub fn new(store: Arc<dyn ReportStore>) -> Self {
        Self { store }
    }

    /// Create a report and return what was actually stored in the database.
    pub async fn insert(&self, report: &Report) -> Result<Report, ReportRepositoryError> {
        let dto = ReportRow::from_report(report);
        let stored = self
            .store
            .insert(&dto)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(stored.into_report_with_events(report.events.clone()))
    }

    /// Fetch a report by job ID. Returns `None` if it doesn't exist.
    pub async fn get(&self, job_id: JobId) -> Result<Option<Report>, ReportRepositoryError> {
        if let Some(dto) = self
            .store
            .get(job_id.0)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?
        {
            Ok(Some(dto.into_report()))
        } else {
            Ok(None)
        }
    }

    /// Fetch a report by job ID and attach events provided by the caller.
    pub async fn get_with_events(
        &self,
        job_id: JobId,
        events: Vec<EventSnapshot>,
    ) -> Result<Option<Report>, ReportRepositoryError> {
        if let Some(dto) = self
            .store
            .get(job_id.0)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?
        {
            Ok(Some(dto.into_report_with_events(events)))
        } else {
            Ok(None)
        }
    }

    /// Update a report and return what was actually stored in the database.
    pub async fn update(&self, report: &Report) -> Result<Report, ReportRepositoryError> {
        let dto = ReportRow::from_report(report);
        let stored = self
            .store
            .update(&dto)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(stored.into_report_with_events(report.events.clone()))
    }

    /// Delete a report by job ID. Returns an error if it doesn't exist.
    pub async fn delete(&self, job_id: JobId) -> Result<(), ReportRepositoryError> {
        self.store
            .delete(job_id.0)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)
    }

    /// Fetch a report by job ID inside an existing transaction.
    pub async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: JobId,
    ) -> Result<Option<Report>, ReportRepositoryError> {
        let row = self
            .store
            .get_tx(tx, job_id.0)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(row.map(ReportRow::into_report))
    }

    /// Create a report inside an existing transaction and return stored data.
    pub async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        report: &Report,
    ) -> Result<Report, ReportRepositoryError> {
        let dto = ReportRow::from_report(report);
        let stored = self
            .store
            .insert_tx(tx, &dto)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(stored.into_report_with_events(report.events.clone()))
    }

    /// Update a report inside an existing transaction and return stored data.
    pub async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        report: &Report,
    ) -> Result<Report, ReportRepositoryError> {
        let dto = ReportRow::from_report(report);
        let stored = self
            .store
            .update_tx(tx, &dto)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)?;

        Ok(stored.into_report_with_events(report.events.clone()))
    }

    /// Delete a report by job ID inside an existing transaction.
    pub async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        job_id: JobId,
    ) -> Result<(), ReportRepositoryError> {
        self.store
            .delete_tx(tx, job_id.0)
            .await
            .map_err(|_| ReportRepositoryError::StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::ReportRepository;
    use crate::domain::entities::job::JobOutcome;
    use crate::domain::entities::report::{EventSnapshot, Report};
    use crate::domain::value_objects::ids::JobId;
    use crate::domain::value_objects::timestamps::Timestamp;
    use crate::infrastructure::db::dto::ReportRow;
    use crate::infrastructure::db::stores::report_store::{ReportRepositoryError, ReportStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    struct DummyStore {
        pub inserted: Mutex<Option<ReportRow>>,
        pub get_result: Mutex<Option<Option<ReportRow>>>,
    }

    impl DummyStore {
        fn new() -> Self {
            Self {
                inserted: Mutex::new(None),
                get_result: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ReportStore for DummyStore {
        async fn get(
            &self,
            _job_id: uuid::Uuid,
        ) -> Result<Option<ReportRow>, ReportRepositoryError> {
            Ok(self.get_result.lock().unwrap().take().unwrap_or(None))
        }

        async fn insert(&self, row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
            *self.inserted.lock().unwrap() = Some(row.clone());
            Ok(row.clone())
        }

        async fn update(&self, _row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
        }

        async fn delete(&self, _job_id: uuid::Uuid) -> Result<(), ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
        }

        async fn insert_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ReportRow,
        ) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
        }

        async fn update_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _row: &ReportRow,
        ) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
        }

        async fn delete_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<(), ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
        }

        async fn get_tx(
            &self,
            _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            _job_id: uuid::Uuid,
        ) -> Result<Option<ReportRow>, ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
        }
    }

    fn sample_report() -> Report {
        let now = Timestamp::now_utc();
        Report {
            job_id: JobId::new(),
            outcome: JobOutcome::Success,
            outcome_reason: None,
            started_at: now,
            finished_at: now,
            duration_ms: 0,
            events: Vec::new(),
        }
    }

    #[tokio::test]
    async fn given_report_when_insert_should_return_stored_report() {
        let store = Arc::new(DummyStore::new());
        let repo = ReportRepository::new(store);
        let report = sample_report();

        let stored = repo.insert(&report).await.unwrap();

        assert_eq!(stored.job_id, report.job_id);
        assert_eq!(stored.outcome, report.outcome);
    }

    #[tokio::test]
    async fn given_report_row_when_get_should_return_report() {
        let store = Arc::new(DummyStore::new());
        let repo = ReportRepository::new(store.clone());
        let now = OffsetDateTime::now_utc();
        let row = ReportRow {
            job_id: uuid::Uuid::new_v4(),
            outcome: "success".to_string(),
            outcome_reason: None,
            started_at: now,
            finished_at: now,
            duration_ms: 0,
            created_at: now,
        };
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let fetched = repo.get(JobId(row.job_id)).await.unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().job_id.0, row.job_id);
    }

    #[tokio::test]
    async fn given_missing_report_when_get_should_return_none() {
        let store = Arc::new(DummyStore::new());
        let repo = ReportRepository::new(store.clone());
        *store.get_result.lock().unwrap() = Some(None);

        let fetched = repo.get(JobId::new()).await.unwrap();

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn given_report_row_when_get_with_events_should_attach_events() {
        let store = Arc::new(DummyStore::new());
        let repo = ReportRepository::new(store.clone());
        let now = OffsetDateTime::now_utc();
        let row = ReportRow {
            job_id: uuid::Uuid::new_v4(),
            outcome: "success".to_string(),
            outcome_reason: None,
            started_at: now,
            finished_at: now,
            duration_ms: 0,
            created_at: now,
        };
        *store.get_result.lock().unwrap() = Some(Some(row.clone()));

        let events = vec![EventSnapshot {
            event_name: crate::domain::entities::event::EventName::JobCreated,
            prev_state: crate::domain::entities::job::JobState::Created,
            next_state: crate::domain::entities::job::JobState::Created,
            timestamp: Timestamp::now_utc(),
        }];

        let fetched = repo
            .get_with_events(JobId(row.job_id), events.clone())
            .await
            .unwrap();

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().events.len(), events.len());
    }
}
