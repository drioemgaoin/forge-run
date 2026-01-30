use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("connection error: {0}")]
    Connection(String),

    #[error("query error: {0}")]
    Query(String),
}

#[async_trait]
pub trait Database: Send + Sync {
    async fn execute(&self, query: &str) -> Result<u64, DatabaseError>;
}
