mod auth;
pub mod dto;
pub mod problem;
pub mod routes;
pub mod state;
pub mod trace;

use crate::interface::http::problem::problem_middleware;
use crate::interface::http::trace::trace_id_middleware;
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
        .layer(middleware::from_fn(problem_middleware))
        .layer(middleware::from_fn(trace_id_middleware))
        .with_state(state)
}
