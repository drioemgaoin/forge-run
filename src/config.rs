use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: Server,
    pub db: Db,
    pub redis: Redis,
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
