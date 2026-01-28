pub mod dto;
pub mod routes;

use axum::Router;

pub fn app() -> Router {
    Router::new().merge(routes::health::router())
}
