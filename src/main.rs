use forge_run::application::context::AppContext;
use forge_run::application::usecases::deliver_webhooks::DeliverWebhooksUseCase;
use forge_run::application::usecases::scheduler::SchedulerUseCase;
use forge_run::application::usecases::worker_manager::WorkerManager;
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
    let ctx = AppContext::new(repos, Arc::new(lifecycle), settings.clone());
    let state = AppState { ctx: Arc::new(ctx) };

    // Step 5: Start the worker manager for background processing.
    let worker_manager = Arc::new(WorkerManager::new(state.ctx.clone(), &settings.workers));
    worker_manager.start().await.expect("start workers");

    // Step 6: Run the scheduler once to handle missed schedules on startup.
    let _ = SchedulerUseCase::run_once(
        &state.ctx,
        forge_run::domain::value_objects::timestamps::Timestamp::now_utc(),
        settings.scheduler.max_batch,
    )
    .await;

    // Step 7: Start the scheduler loop in the background.
    let (scheduler_tx, scheduler_rx) = tokio::sync::watch::channel(false);
    let _ = scheduler_tx;
    let scheduler_ctx = state.ctx.clone();
    let scheduler_poll = time::Duration::milliseconds(settings.scheduler.poll_interval_ms as i64);
    let scheduler_limit = settings.scheduler.max_batch;
    tokio::spawn(async move {
        let _ = SchedulerUseCase::run_loop(
            &scheduler_ctx,
            scheduler_poll,
            scheduler_limit,
            scheduler_rx,
        )
        .await;
    });

    // Step 8: Start the webhook delivery loop in the background.
    let (webhook_tx, webhook_rx) = tokio::sync::watch::channel(false);
    let _ = webhook_tx;
    let webhook_ctx = state.ctx.clone();
    let webhook_poll =
        time::Duration::milliseconds(settings.webhook_delivery.poll_interval_ms as i64);
    let webhook_limit = settings.webhook_delivery.batch_size;
    tokio::spawn(async move {
        let _ =
            DeliverWebhooksUseCase::run_loop(&webhook_ctx, webhook_poll, webhook_limit, webhook_rx)
                .await;
    });

    // Step 9: Build the HTTP app.
    let app = http::app(state);
    let bind_addr = format!("{}:{}", settings.server.host, settings.server.port);

    // Step 10: Bind and serve.
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind server");

    axum::serve(listener, app).await.expect("serve");
}
