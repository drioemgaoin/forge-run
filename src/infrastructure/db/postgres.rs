use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(1800),
        }
    }
}

#[derive(Clone)]
pub struct Postgres {
    pool: PgPool,
}

impl Postgres {
    pub async fn connect(config: &PostgresConfig) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.connect_timeout)
            .idle_timeout(config.idle_timeout)
            .max_lifetime(config.max_lifetime)
            .connect(&config.database_url)
            .await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn healthcheck(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}
