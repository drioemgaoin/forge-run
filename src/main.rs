use forge_run::application::context::AppContext;
use forge_run::config;
use forge_run::domain::services::job_lifecycle::JobLifecycle;
use forge_run::infrastructure::db::postgres::PostgresDatabase;
use forge_run::infrastructure::db::repositories::Repositories;
use forge_run::interface::http;
use forge_run::interface::http::state::AppState;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Step 1: Load configuration.
    let settings = config::load().expect("load config");

    // Step 2: Connect to the database.
    let db = Arc::new(
        PostgresDatabase::connect(&settings.db.url)
            .await
            .expect("connect database"),
    );

    // Step 3: Build repositories and domain services.
    let repos = Repositories::postgres(db.clone());
    let lifecycle = JobLifecycle::new(repos.clone());

    // Step 4: Assemble shared application context and HTTP state.
    let ctx = AppContext::new(repos, Arc::new(lifecycle));
    let state = AppState {
        ctx: Arc::new(ctx),
        settings: settings.clone(),
    };

    // Step 5: Build the HTTP app.
    let app = http::app(state);
    let bind_addr = format!("{}:{}", settings.server.host, settings.server.port);

    // Step 6: Bind and serve.
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind server");

    axum::serve(listener, app).await.expect("serve");
}
