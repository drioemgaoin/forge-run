use std::sync::Arc;

use crate::config::Settings;
use crate::infrastructure::db::database::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Database>,
    pub settings: Settings,
}
