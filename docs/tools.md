# ForgeRun Tooling & Crates

This file lists the recommended tools and Rust crates to implement ForgeRun end‑to‑end.
All links are official documentation pages.

## Rust Toolchain

- rustup (toolchain manager)
- cargo (build/test)
- rustfmt (formatting)
- clippy (linting)

Docs:

```
https://rustup.rs/
https://doc.rust-lang.org/cargo/
https://doc.rust-lang.org/rustfmt/
https://doc.rust-lang.org/clippy/
```

## Runtime & Async

- tokio (async runtime, timers, tasks)

Docs:

```
https://tokio.rs/
https://docs.rs/tokio
```

## HTTP Server

- axum (HTTP API)
- tower-http (middleware: trace, timeout, compression)

Docs:

```
https://docs.rs/axum
https://docs.rs/tower-http
```

## HTTP Client (Webhooks)

- reqwest (HTTP client for webhook delivery)

Docs:

```
https://docs.rs/reqwest
```

## Serialization & JSON

- serde (Serialize/Deserialize)
- serde_json (JSON encoding/decoding)

Docs:

```
https://docs.rs/serde
https://docs.rs/serde_json
```

## IDs & Time

- uuid (job_id, event_id, client_id)
- time (UTC timestamps, ISO‑8601 formatting)

Docs:

```
https://docs.rs/uuid
https://docs.rs/time
```

## Errors

- thiserror (typed error enums)
- anyhow (application error propagation)

Docs:

```
https://docs.rs/thiserror
https://docs.rs/anyhow
```

## Storage

- sqlx (async SQL access; use SQLite for local, self‑hosted runs)

Docs:

```
https://docs.rs/sqlx
```

## Observability (Logs/Tracing)

- tracing (structured logs/spans)
- tracing-subscriber (formatting, filtering)
- tracing-opentelemetry (optional export to OTEL)
- opentelemetry + opentelemetry_sdk (optional)

Docs:

```
https://docs.rs/tracing
https://docs.rs/tracing-subscriber
https://docs.rs/tracing-opentelemetry
https://docs.rs/opentelemetry
https://docs.rs/opentelemetry_sdk
```

## Metrics

- metrics (metrics facade)
- metrics-exporter-prometheus (Prometheus endpoint)

Docs:

```
https://docs.rs/metrics
https://docs.rs/metrics-exporter-prometheus
https://metrics.rs/
```

## Testing (TDD)

- proptest (property‑based tests)
- wiremock (mock webhook receiver)
- insta (snapshot testing for error payloads / responses)

Docs:

```
https://docs.rs/proptest
https://docs.rs/wiremock
https://docs.rs/insta
https://insta.rs/docs/
```

## Optional Quality Tools

- cargo-audit (dependency vulnerability scanning)
- cargo-deny (license and duplicate dependency checks)
- cargo-llvm-cov (coverage)

Docs:

```
https://docs.rs/cargo-audit
https://docs.rs/cargo-deny
https://docs.rs/cargo-llvm-cov
```
