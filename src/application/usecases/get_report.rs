// Use case: get_report.

use crate::application::context::AppContext;
use crate::domain::entities::report::{EventSnapshot, Report};
use crate::domain::value_objects::ids::JobId;
use crate::infrastructure::db::dto::EventRow;

/// Fetches a job report and its events.
pub struct GetReportUseCase;

#[derive(Debug)]
pub enum GetReportError {
    NotFound,
    Storage(String),
}

impl GetReportUseCase {
    /// Get a report by job ID (including events).
    pub async fn execute(ctx: &AppContext, job_id: JobId) -> Result<Report, GetReportError> {
        // Step 1: Fetch the report row.
        let report = ctx
            .repos
            .report
            .get(job_id)
            .await
            .map_err(|e| GetReportError::Storage(format!("{e:?}")))?;

        // Step 2: Return NotFound when missing.
        let report = report.ok_or(GetReportError::NotFound)?;

        // Step 3: Fetch all events for the job.
        let event_rows = ctx
            .repos
            .event
            .list_by_job_id(job_id)
            .await
            .map_err(|e| GetReportError::Storage(format!("{e:?}")))?;

        // Step 4: Convert event rows to snapshots.
        let events = event_rows
            .into_iter()
            .map(EventRow::into_event)
            .map(|event| EventSnapshot {
                event_name: event.event_name,
                prev_state: event.prev_state,
                next_state: event.next_state,
                timestamp: event.timestamp,
            })
            .collect::<Vec<_>>();

        // Step 5: Return report with events.
        let mut report = report;
        report.events = events;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::{GetReportError, GetReportUseCase};
    use crate::application::context::test_support::test_context;
    use crate::domain::entities::event::EventName;
    use crate::domain::entities::job::JobOutcome;
    use crate::domain::value_objects::ids::JobId;
    use crate::infrastructure::db::dto::{EventRow, ReportRow};
    use crate::infrastructure::db::stores::event_store::{EventRepositoryError, EventStore};
    use crate::infrastructure::db::stores::report_store::{ReportRepositoryError, ReportStore};
    use async_trait::async_trait;
    use std::sync::Mutex;
    use time::OffsetDateTime;

    struct DummyReportStore {
        row: Mutex<Option<ReportRow>>,
    }

    struct DummyEventStore {
        rows: Mutex<Vec<EventRow>>,
    }

    #[async_trait]
    impl ReportStore for DummyReportStore {
        async fn get(
            &self,
            _job_id: uuid::Uuid,
        ) -> Result<Option<ReportRow>, ReportRepositoryError> {
            Ok(self.row.lock().unwrap().clone())
        }

        async fn insert(&self, _row: &ReportRow) -> Result<ReportRow, ReportRepositoryError> {
            Err(ReportRepositoryError::InvalidInput)
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

    #[async_trait]
    impl EventStore for DummyEventStore {
        async fn get(
            &self,
            _event_id: uuid::Uuid,
        ) -> Result<Option<EventRow>, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
        }

        async fn insert(&self, _row: &EventRow) -> Result<EventRow, EventRepositoryError> {
            Err(EventRepositoryError::InvalidInput)
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
            Err(EventRepositoryError::InvalidInput)
        }

        async fn list_by_job_id(
            &self,
            _job_id: uuid::Uuid,
        ) -> Result<Vec<EventRow>, EventRepositoryError> {
            Ok(self.rows.lock().unwrap().clone())
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

    #[tokio::test]
    async fn given_existing_report_when_execute_should_return_report_with_events() {
        let job_id = JobId::new();
        let now = OffsetDateTime::now_utc();
        let report_row = ReportRow {
            job_id: job_id.0,
            outcome: "success".to_string(),
            outcome_reason: None,
            started_at: now,
            finished_at: now,
            duration_ms: 1,
            created_at: now,
        };
        let event_row = EventRow {
            id: uuid::Uuid::new_v4(),
            job_id: job_id.0,
            event_name: "job_created".to_string(),
            prev_state: "created".to_string(),
            next_state: "created".to_string(),
            occurred_at: now,
        };

        let report_store = DummyReportStore {
            row: Mutex::new(Some(report_row)),
        };
        let event_store = DummyEventStore {
            rows: Mutex::new(vec![event_row]),
        };

        let mut ctx = test_context();
        ctx.repos.report = std::sync::Arc::new(
            crate::infrastructure::db::repositories::report_repository::ReportRepository::new(
                std::sync::Arc::new(report_store),
            ),
        );
        ctx.repos.event = std::sync::Arc::new(
            crate::infrastructure::db::repositories::event_repository::EventRepository::new(
                std::sync::Arc::new(event_store),
            ),
        );

        let report = GetReportUseCase::execute(&ctx, job_id).await.unwrap();

        assert_eq!(report.job_id, job_id);
        assert_eq!(report.outcome, JobOutcome::Success);
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].event_name, EventName::JobCreated);
    }

    #[tokio::test]
    async fn given_missing_report_when_execute_should_return_not_found() {
        let report_store = DummyReportStore {
            row: Mutex::new(None),
        };
        let event_store = DummyEventStore {
            rows: Mutex::new(Vec::new()),
        };

        let mut ctx = test_context();
        ctx.repos.report = std::sync::Arc::new(
            crate::infrastructure::db::repositories::report_repository::ReportRepository::new(
                std::sync::Arc::new(report_store),
            ),
        );
        ctx.repos.event = std::sync::Arc::new(
            crate::infrastructure::db::repositories::event_repository::EventRepository::new(
                std::sync::Arc::new(event_store),
            ),
        );

        let result = GetReportUseCase::execute(&ctx, JobId::new()).await;

        assert!(matches!(result, Err(GetReportError::NotFound)));
    }
}
