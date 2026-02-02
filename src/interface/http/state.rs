use std::sync::Arc;

use crate::application::context::AppContext;
use metrics_exporter_prometheus::PrometheusHandle;

#[derive(Clone)]
pub struct AppState {
    pub ctx: Arc<AppContext>,
    pub metrics: Option<PrometheusHandle>,
}
