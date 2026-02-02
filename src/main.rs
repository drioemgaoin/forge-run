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
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    // Step 1: Load configuration.
    let settings = config::load().expect("load config");

    // Step 0: Initialize structured logging and tracing.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_path = settings
        .observability
        .log_file_path
        .clone()
        .filter(|p| !p.trim().is_empty());
    let (writer, _log_guard) = if let Some(path) = log_path {
        let path = std::path::PathBuf::from(path);
        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let file_name = path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("forge-run.log"));
        let appender = tracing_appender::rolling::never(dir, file_name);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        (BoxMakeWriter::new(non_blocking), Some(guard))
    } else {
        (BoxMakeWriter::new(std::io::stdout), None)
    };
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(writer);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);
    if settings.observability.enable_tracing {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(settings.observability.otlp_endpoint.clone());
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(opentelemetry_sdk::trace::config().with_resource(
                opentelemetry_sdk::Resource::new(vec![KeyValue::new(
                    "service.name",
                    settings.observability.service_name.clone(),
                )]),
            ))
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .expect("install otlp tracer");
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        registry.with(otel_layer).init();
    } else {
        registry.init();
    }

    // Step 0b: Initialize metrics exporter.
    let metrics_handle = if settings.observability.enable_metrics {
        metrics_exporter_prometheus::PrometheusBuilder::new()
            .install_recorder()
            .ok()
    } else {
        None
    };

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
    let state = AppState {
        ctx: Arc::new(ctx),
        metrics: metrics_handle,
    };

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
