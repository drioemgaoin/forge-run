mod auth;
pub mod dto;
pub mod problem;
pub mod routes;
pub mod state;
pub mod trace;

use crate::interface::http::problem::problem_middleware;
use crate::interface::http::trace::{request_log_middleware, trace_id_middleware};
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
        .merge(routes::metrics::router())
        .merge(routes::ready::router())
        .merge(routes::health::router::<AppState>())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .layer(middleware::from_fn(problem_middleware))
        .layer(middleware::from_fn(request_log_middleware))
        .layer(middleware::from_fn(trace_id_middleware))
        .with_state(state)
}
