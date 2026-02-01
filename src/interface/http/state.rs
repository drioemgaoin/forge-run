use std::sync::Arc;

use crate::application::context::AppContext;

#[derive(Clone)]
pub struct AppState {
    pub ctx: Arc<AppContext>,
}
