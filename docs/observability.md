# Observability

This project exposes structured logs, traces, and Prometheus metrics.  
It also ships a local observability stack (Grafana + Prometheus + Tempo + Loki).

## What is implemented

### Logs
- JSON logs (stdout by default).
- `x-request-id` propagated into logs and error responses.
- Job-level logs include `job_id`, `client_id`, and state transitions.

### Tracing
- OpenTelemetry spans (OTLP exporter).
- Key use cases create spans: submit job, scheduler, worker, webhook delivery.

### Metrics (Prometheus)
- `http_requests_total` (labels: method, status)
- `http_request_duration_ms` (histogram)
- `jobs_submitted_total`
- `jobs_processed_total`
- `jobs_succeeded_total`
- `jobs_failed_total`
- `jobs_canceled_total`
- `scheduled_jobs_total`
- `queue_depth`
- `webhook_deliveries_processed`
- `webhook_deliveries_delivered`
- `webhook_deliveries_failed`
- `webhook_delivery_latency_ms` (histogram)

### Readiness/Health
- `/health` for liveness.
- `/ready` for DB readiness.
- `/metrics` for Prometheus scraping.

## Configuration

`config/default.toml`:

```toml
[observability]
service_name = "forge-run"
enable_tracing = true
otlp_endpoint = "http://127.0.0.1:4317"
enable_metrics = true
log_file_path = ""
```

- `log_file_path`: optional file sink for logs.  
  If empty, logs stay on stdout.

## Local UI (Grafana)

Start the stack:

```bash
docker compose -f docker-compose.observability.yml up -d
```

Open:
- Grafana: `http://localhost:3000` (admin / admin)
- Prometheus: `http://localhost:9090`
- Tempo: `http://localhost:3200`
- Loki: `http://localhost:3100`

### Prometheus scrape target
By default, Prometheus scrapes `host.docker.internal:8080/metrics`.
If your app runs on a different host/port, update:

```
config/observability/prometheus.yml
```

### Logs into Loki
Promtail expects logs at:
```
/var/log/forge-run/forge-run.log
```

To enable file logging, set:

```
FORGERUN__OBSERVABILITY__LOG_FILE_PATH=/var/log/forge-run/forge-run.log
```

Then run promtail with the provided config (already in the docker compose).

## Tracing

The OTEL collector listens on `4317` and forwards to Tempo.
Tracing is enabled by default in `config/default.toml`.

You can disable it by setting:

```
FORGERUN__OBSERVABILITY__ENABLE_TRACING=false
```

## SLA / SLI / KPI (suggested)

### Core SLIs
- **Job success rate** = `jobs_succeeded_total / jobs_processed_total`
- **Webhook delivery success rate** = `webhook_deliveries_delivered / webhook_deliveries_processed`
- **Queue latency** = time between `JobCreated` and `JobQueued`
- **Execution latency** = time between `JobQueued` and `JobSucceeded/Failed`
- **HTTP latency** = `http_request_duration_ms` P50/P95/P99

### Example SLOs (you can tune)
- 99.9% HTTP requests under 500ms
- 99.5% jobs complete within 2s for instant jobs
- 99.0% webhook deliveries succeed within 30s

### KPIs
- Jobs processed per minute
- Queue depth over time
- Webhook delivery retry rate

## Support workflow

When a user reports an issue:
1. Ask for `x-request-id` or `job_id`.
2. Query logs by `trace_id` or `job_id`.
3. Inspect traces in Tempo (same trace id).
4. Check Prometheus/Grafana for spikes in errors or latency.
