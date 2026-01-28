# ForgeRun

## Table of Contents

- [Overview](#overview)
- [Architecture Overview](#architecture-overview)
- [Use Cases (Application Layer)](#use-cases-application-layer)
- [Specification](#specification)
- [Plan & Milestones](#plan--milestones)
- [Tools & Crates](#tools--crates)
- [Docs Index](#docs-index)

## Overview

ForgeRun is a self‑hosted job execution API used to practice real‑world backend concerns: state machines, scheduling, retries, observability, and failure handling.

## Architecture Overview

This project follows a single‑crate Clean Architecture layout. Each folder has a single responsibility and is easy to locate via IDE/intellisense.

### src/

- `domain/` — Pure business rules (entities, value objects, workflows)
  - `entities/` — One file per entity (Job, Event, Report, Client)
  - `value_objects/` — IDs, timestamps, and small types
  - `workflows/` — State machine rules and domain workflows

- `application/` — Use cases (orchestrate domain + infrastructure)
  - `usecases/` — One file per use case (submit job, retry, cancel, etc.)
  - `contracts/` — Interfaces used by use cases (repositories, policies)

- `infrastructure/` — Adapters to external systems
  - `db/` — Postgres repositories
  - `cache/` — Redis cache
  - `queue/` — Queue adapter and persistence

- `interface/` — Input/output boundaries
  - `http/` — Axum API
    - `routes/` — One file per route group (jobs, reports, clients, api keys, etc.)
    - `dto/` — Request/response DTOs
    - `middleware/` — Auth, logging, etc.

- `lib.rs` — Exposes the layers
- `main.rs` — App entrypoint (HTTP server)

## Use Cases (Application Layer)

Each use case is called by an HTTP route (client request) or by internal schedulers/workers.
It contains orchestration logic and calls domain + infrastructure.

### Client‑initiated (HTTP)

- `submit_job` — Called by `POST /jobs`. Validates input, generates job definition, persists, enqueues, emits events.
- `cancel_job` — Called by `POST /jobs/{job_id}/cancel`. Validates state, updates job, emits cancel event.
- `retry_job` — Called by `POST /jobs/{job_id}/retry`. Checks FAILED state, increments attempt, re‑queues.
- `get_job` — Called by `GET /jobs/{job_id}`. Loads job and returns current state/outcome.
- `get_report` — Called by `GET /jobs/{job_id}/report`. Loads report + events for a completed job.
- `create_client` — Called by `POST /clients`. Creates a new client record.
- `create_api_key` — Called by `POST /clients/{client_id}/keys`. Creates or returns key.
- `renew_api_key` — Called by `POST /clients/{client_id}/keys/renew`. Rotates key.
- `revoke_api_key` — Called by `POST /clients/{client_id}/keys/revoke`. Revokes key.
- `register_webhook` — Called by `POST /webhooks`. Stores client default webhook.
- `unregister_webhook` — Called by `DELETE /webhooks/{webhook_id}`. Removes client default webhook.

### System‑initiated (internal)

- `scheduler` — Moves scheduled jobs to QUEUED when `execution_at` is reached.
- `worker` — Claims QUEUED jobs, executes work_kind, updates state/outcome, emits events, triggers webhooks.

## Specification

See the full technical specification in [docs/specification.md](docs/specification.md).

## Plan & Milestones

See the implementation plan in [docs/plan.md](docs/plan.md).

## Tools & Crates

See the tooling list in [docs/tools.md](docs/tools.md).

## Running the Project

Prerequisites:
- Rust toolchain installed (rustup + cargo)
- Postgres running locally
- Redis running locally

Environment (example):
- `DATABASE_URL=postgres://...`
- `REDIS_URL=redis://...`

Run locally:

```sh
cargo run
```

Run tests:

```sh
cargo test
```

Format and lint:

```sh
cargo fmt
cargo clippy
```

## Docs Index

- [docs/specification.md](docs/specification.md) — full system specification
- [docs/plan.md](docs/plan.md) — milestones and tasks
- [docs/tools.md](docs/tools.md) — tools, crates, official docs
