use forge_run::interface::http;

#[tokio::main]
async fn main() {
    let app = http::app();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("bind server");

    axum::serve(listener, app).await.expect("serve");
}
