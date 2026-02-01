# ForgeRun Implementation Plan (Scrum)

Assumptions:

- Solo developer, beginner in Rust.
- Part-time pace: ~10–15 hours/week.
- TDD applied to all core logic.
- Estimates include learning time, debugging, and refactors.

Legend:

- Est = estimate in hours
- Tests = required test types for the task

## Milestone 1 — Project setup & foundations (Est: 10–16h)

Goal: runnable skeleton, tooling, test harness ready.

Tasks:

- Initialize workspace structure, crates/modules, and basic CI (Est: 3–5h) | Tests: none
- Add test tooling (unit + integration + property-based + golden) (Est: 2–3h) | Tests: meta
- Define core domain types and enums in Rust (Est: 3–5h) | Tests: unit
- Create minimal HTTP server skeleton with routing (Est: 2–3h) | Tests: integration (health endpoint)

## Milestone 2 — Domain model + state machine (Est: 18–28h)

Goal: Job/Event/Report models, rules, and transitions with tests.

Tasks:

- Implement Job state machine with allowed transitions (Est: 6–8h) | Tests: unit + property
- Implement Event model + lifecycle event generation (Est: 4–6h) | Tests: unit
- Implement Report generation model (Est: 4–6h) | Tests: unit
- Add ID generation + timestamps (Est: 2–4h) | Tests: unit
- Build domain validation rules (Est: 2–4h) | Tests: unit

## Milestone 3 — Storage layer (Est: 20–30h)

Goal: persistence for jobs/events/reports/clients and API key fields on client.

Tasks:

- Choose storage backend (Postgres) and schema (Est: 2–4h) | Tests: none
- Implement repositories for Job/Event/Report/Client (Est: 10–14h) | Tests: integration
- Ensure atomicity for multi-write flows (Est: 4–6h) | Tests: integration
- Add retention cleanup routines (Est: 4–6h) | Tests: integration

## Milestone 4 — API contract (Est: 24–36h)

Goal: Implement all endpoints with RFC 7807 errors and idempotency.

Tasks:

- Implement auth middleware + API key management (Est: 6–8h) | Tests: integration
- Implement submit/cancel/retry/get/report endpoints (Est: 10–14h) | Tests: integration + unit
- Implement idempotency behavior (Est: 4–6h) | Tests: unit + integration
- Implement error model + RFA codes (Est: 4–6h) | Tests: integration

## Milestone 5 — Queue + worker loop (Est: 24–36h)

Goal: Job execution pipeline with leases, heartbeat, retry.

Tasks:

- Implement queue persistence and FIFO behavior (Est: 6–8h) | Tests: integration
- Implement worker lease/heartbeat + requeue (Est: 8–12h) | Tests: integration
- Implement retry policy + backoff (Est: 4–6h) | Tests: unit + integration
- Implement job execution engine using work_kind catalog (Est: 6–10h) | Tests: unit + integration

## Milestone 6 — Scheduling + deferred jobs (Est: 12–20h)

Goal: Scheduled execution correctness under restart.

Tasks:

- Implement scheduler loop (Est: 4–6h) | Tests: integration
- Handle missed schedules on restart (Est: 4–6h) | Tests: integration
- Add time-skew handling + validation (Est: 2–4h) | Tests: unit

## Milestone 7 — Webhooks & notification (Est: 12–20h)

Goal: Register webhooks, deliver events with retries.

Tasks:

- Webhook registration + storage (Est: 3–5h) | Tests: integration
- Delivery engine + retry policy (Est: 6–10h) | Tests: integration
- Delivery observability (Est: 3–5h) | Tests: integration

## yes i want (Est: 16–24h)

Goal: metrics, logs, tracing, and dashboards.

Tasks:

- Implement required log fields and structured logging (Est: 4–6h) | Tests: unit
- Implement metrics (counters/gauges/histograms) (Est: 6–8h) | Tests: integration
- Implement tracing spans for core flows (Est: 4–6h) | Tests: integration
- Add health/readiness endpoints (Est: 2–4h) | Tests: integration

## Milestone 9 — Load/chaos simulator (Est: 16–24h)

Goal: validate behavior under stress.

Tasks:

- Build traffic generator (Est: 6–8h) | Tests: integration
- Add chaos behaviors (timeouts, failures, retries) (Est: 6–8h) | Tests: integration
- Validate SLA/limits under load (Est: 4–8h) | Tests: integration

## Milestone 10 — Hardening & polish (Est: 16–24h)

Goal: edge cases, doc polish, test coverage.

Tasks:

- Add property-based tests for state transitions (Est: 4–6h) | Tests: property
- Add end-to-end tests for each work_kind (Est: 6–8h) | Tests: e2e
- Add upgrade/restart tests (Est: 3–5h) | Tests: integration
- Polish docs and examples (Est: 3–5h) | Tests: none

## Test Strategy (TDD)

- For each feature: write failing test → implement → refactor.
- Test pyramid:
  - Unit tests for pure logic (state machine, validation, work_kind mapping).
  - Integration tests for DB + HTTP endpoints.
  - Property-based tests for transitions and idempotency.
  - End-to-end tests for job lifecycle flows.

## Estimated Timeline

- Total: ~168–258 hours
- At 40h/week: ~4–7 weeks

If you want, I can tailor estimates to your weekly time budget and preferred stack (Actix/Axum, SQLite/Postgres, etc.).
