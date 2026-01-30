use crate::infrastructure::db::database::{Database, DatabaseError};
use async_trait::async_trait;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::future::Future;
use std::pin::Pin;

pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
    pub async fn connect(url: &str) -> Result<Self, DatabaseError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn with_conn<T, E, F>(&self, f: F) -> Result<T, E>
    where
        for<'c> F: FnOnce(
            &'c mut sqlx::PgConnection,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>,
        E: From<DatabaseError>,
    {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;
        f(&mut conn).await
    }

    pub async fn with_tx<T, F>(&self, f: F) -> Result<T, DatabaseError>
    where
        for<'c> F: FnOnce(
            &'c mut sqlx::Transaction<'_, sqlx::Postgres>,
        )
            -> Pin<Box<dyn Future<Output = Result<T, DatabaseError>> + Send + 'c>>,
    {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DatabaseError::Connection(e.to_string()))?;
        let result = f(&mut tx).await;
        match result {
            Ok(value) => {
                tx.commit()
                    .await
                    .map_err(|e| DatabaseError::Query(e.to_string()))?;
                Ok(value)
            }
            Err(err) => {
                let _ = tx.rollback().await;
                Err(err)
            }
        }
    }
}

#[async_trait]
impl Database for PostgresDatabase {
    async fn execute(&self, query: &str) -> Result<u64, DatabaseError> {
        let result = sqlx::query(query)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::PostgresDatabase;
    use crate::infrastructure::db::database::{Database, DatabaseError};

    fn test_db_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn given_valid_database_url_when_execute_should_return_rows_affected() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = PostgresDatabase::connect(&url).await.unwrap();

        let rows = db.execute("SELECT 1").await.unwrap();
        assert_eq!(rows, 0);
    }

    #[tokio::test]
    async fn given_connection_when_with_conn_should_run_query_and_return_value() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = PostgresDatabase::connect(&url).await.unwrap();

        let value: i64 = db
            .with_conn(|conn| {
                Box::pin(async move {
                    let v = sqlx::query_scalar::<_, i64>("SELECT 1")
                        .fetch_one(conn)
                        .await
                        .map_err(|e| DatabaseError::Query(e.to_string()))?;
                    Ok::<i64, DatabaseError>(v)
                })
            })
            .await
            .unwrap();

        assert_eq!(value, 1);
    }

    #[tokio::test]
    async fn given_transaction_when_success_should_commit_and_return_value() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = PostgresDatabase::connect(&url).await.unwrap();

        let result = db.with_tx(|_tx| Box::pin(async { Ok(1) })).await.unwrap();

        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn given_transaction_when_error_should_rollback_and_return_error() {
        let Some(url) = test_db_url() else {
            return;
        };
        let db = PostgresDatabase::connect(&url).await.unwrap();

        let result: Result<(), DatabaseError> = db
            .with_tx(|_tx| Box::pin(async { Err(DatabaseError::Query("boom".to_string())) }))
            .await;

        assert!(matches!(result, Err(DatabaseError::Query(_))));
    }
}
