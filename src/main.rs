use forge_run::config;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let settings = config::load().expect("load config");
    let db = Arc::new(
        PostgresDatabase::connect(&settings.db.url)
            .await
            .expect("connect database"),
    );
    let state = AppState {
        db: db.clone(),
        settings: settings.clone(),
    };
    let app = http::app(state);
    let bind_addr = format!("{}:{}", settings.server.host, settings.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind server");

    axum::serve(listener, app).await.expect("serve");
}
