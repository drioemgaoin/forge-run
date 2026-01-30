pub mod dto;
pub mod routes;
pub mod state;

use axum::Router;
use state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(routes::health::router::<AppState>())
        .with_state(state)
}
