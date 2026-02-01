use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: Server,
    pub db: Db,
    pub redis: Redis,
    pub workers: Workers,
    pub scheduler: Scheduler,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Db {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Redis {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Workers {
    pub default_count: usize,
    pub max_count: usize,
    pub poll_interval_ms: u64,
    pub lease_timeout_seconds: u64,
    pub scale_interval_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Scheduler {
    pub poll_interval_ms: u64,
    pub max_batch: u32,
    pub skew_seconds: i64,
    pub tolerance_ms: u64,
}

/// Load settings from `config/default.toml`, `config/<env>.toml`, and env overrides.
pub fn load() -> Result<Settings, config::ConfigError> {
    let env_name = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    config::Config::builder()
        .add_source(config::File::with_name("config/default"))
        .add_source(config::File::with_name(&format!("config/{env_name}")).required(false))
        .add_source(config::Environment::with_prefix("FORGERUN").separator("__"))
        .build()?
        .try_deserialize()
}
