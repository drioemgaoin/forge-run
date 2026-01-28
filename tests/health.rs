use axum::body::Body;
use axum::http::{Request, StatusCode};
use forge_run::interface::http;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_endpoint_works() {
    let response = http::app()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
