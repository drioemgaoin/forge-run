use std::sync::Arc;

use crate::application::context::AppContext;
use crate::config::Settings;

#[derive(Clone)]
pub struct AppState {
    pub ctx: Arc<AppContext>,
    pub settings: Settings,
}
