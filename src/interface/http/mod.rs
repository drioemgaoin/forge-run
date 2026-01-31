mod auth;
pub mod dto;
pub mod problem;
pub mod routes;
pub mod state;

use axum::Router;
use axum::middleware;
use state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(routes::client::router())
        .merge(routes::api_key::router())
        .merge(routes::job::router())
        .merge(routes::report::router())
        .merge(routes::webhook::router())
        .merge(routes::health::router::<AppState>())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}
