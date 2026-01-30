use std::sync::Arc;

use crate::config::Settings;
use crate::infrastructure::db::postgres::PostgresDatabase;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<PostgresDatabase>,
    pub settings: Settings,
}
