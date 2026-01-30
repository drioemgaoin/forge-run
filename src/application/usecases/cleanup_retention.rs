// Use case: cleanup_retention.

use crate::infrastructure::db::database::Database;
use crate::infrastructure::db::postgres::PostgresDatabase;
use std::sync::Arc;

/// Deletes data that is past its retention window.
pub struct CleanupRetentionUseCase {
    pub db: Arc<PostgresDatabase>,
}

#[derive(Debug)]
pub enum CleanupRetentionError {
    Storage(String),
}

#[derive(Debug, Clone, Copy)]
pub struct CleanupRetentionResult {
    pub jobs_deleted: u64,
    pub api_keys_deleted: u64,
}

impl CleanupRetentionUseCase {
    /// Remove expired jobs (final state older than 30 days) and revoked API keys older than 90 days.
    pub async fn execute(&self) -> Result<CleanupRetentionResult, CleanupRetentionError> {
        // Step 1: Delete jobs in a final state past retention.
        let jobs_deleted = self
            .db
            .execute(
                "DELETE FROM jobs
                 WHERE state IN ('succeeded','failed','canceled')
                   AND updated_at < NOW() - INTERVAL '30 days'",
            )
            .await
            .map_err(|e| CleanupRetentionError::Storage(format!("{e:?}")))?;

        // Step 2: Delete revoked API keys past retention.
        let api_keys_deleted = self
            .db
            .execute(
                "DELETE FROM api_keys
                 WHERE revoked_at IS NOT NULL
                   AND revoked_at < NOW() - INTERVAL '90 days'",
            )
            .await
            .map_err(|e| CleanupRetentionError::Storage(format!("{e:?}")))?;

        // Step 3: Return deletion counts.
        Ok(CleanupRetentionResult {
            jobs_deleted,
            api_keys_deleted,
        })
    }
}
