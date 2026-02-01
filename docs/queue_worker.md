# Queue + Worker System

This document explains **why** the queue/worker system exists, **how** it works, and **how it scales** in this project.  
You should be able to read this without any prior knowledge of the codebase.

---

## 1) What problem are we solving?

We want the system to:
- accept jobs at any time (24/7),
- process them as fast as possible,
- survive crashes or restarts,
- and scale up/down without losing or duplicating work.

The core idea is: **the queue must be persistent**, and **workers must coordinate safely**.

---

## 2) Why the queue is stored in Postgres

We do **not** keep the queue in memory because:
- memory is lost on crash/restart,
- multiple workers need a shared source of truth,
- we need consistent ordering (FIFO),
- and we want to re-queue jobs if a worker dies.

So the queue is modeled in the `jobs` table:
- `state = queued` → the job is waiting
- `state = assigned` → a worker claimed it
- `state = running` → it is actively executing

The queue is **global** and **FIFO by creation time**.

---

## 3) What is a lease (and why it exists)?

A **lease** is temporary ownership of a job by a worker.

When a worker claims a job, it sets:
- `lease_owner` → which worker owns it
- `lease_expires_at` → when ownership expires
- `heartbeat_at` → last time the worker said “I’m alive”

Why?
- If a worker crashes, its lease eventually expires.
- Another worker can safely re-queue and take the job.

This prevents **duplicate processing** and **stuck jobs**.

---

## 4) How a worker processes a job

Each worker follows the same steps:

1. **Claim** the next queued job (FIFO).
2. **Assign** a lease (worker id + expiration).
3. Transition job to `running`.
4. Execute deterministic logic based on `work_kind`.
5. Transition to `succeeded`, `failed`, or `canceled`.
6. Emit events and create a report.

If the job fails and is retryable:
- it gets re-queued with backoff
- and will be picked up again later.

---

## 5) How retries work (backoff)

Retry behavior is controlled by a **retry policy**:
- max retries
- exponential backoff
- jitter to spread load

When a retry happens:
1. `attempt` is incremented
2. `executed_at` is set to “now + backoff”
3. job is moved back to `queued`

The queue only gives jobs that are due (execution time <= now).

---

## 6) Work kind catalog (deterministic execution)

Each job has a `work_kind` which maps to a deterministic behavior:
- duration
- outcome (success/fail/cancel)
- retryable or not

This guarantees consistent results for the same input.

---

## 7) How scaling works (auto scale up/down)

We run a **Worker Manager** that:

1. Starts a **default** number of workers at app startup.
2. Periodically checks **queue depth** (how many jobs are waiting).
3. Scales workers up or down:
   - Desired workers = queue depth (bounded)
   - min = default_count
   - max = max_count

This makes the system **always on**, and responsive to traffic:
- High traffic → more workers
- Low traffic → fewer workers

---

## 8) Configuration (defaults)

These are controlled by `config/default.toml`:

```
[workers]
default_count = 2
max_count = 8
poll_interval_ms = 250
lease_timeout_seconds = 30
scale_interval_ms = 1000
```

Meaning:
- `default_count`: workers started at boot
- `max_count`: hard limit (protect your machine)
- `poll_interval_ms`: how often each worker checks queue
- `lease_timeout_seconds`: lease expiry for crash recovery
- `scale_interval_ms`: how often scaling decisions happen

---

## 9) What happens during crash/restart

If the app or a worker crashes:
- queued jobs remain in the DB
- assigned jobs eventually have expired leases
- the requeue flow puts them back in the queue

So the system recovers **without losing jobs**.

---

## 10) Summary

This design gives us:
- **durable queue** (Postgres)
- **safe coordination** (leases)
- **automatic scaling**
- **resilience to crashes**
- **deterministic execution**

It is simple, maintainable, and works 24/7.
