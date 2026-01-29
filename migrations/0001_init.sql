-- Initial PostgreSQL schema for ForgeRun.

-- Enums
CREATE TYPE job_type AS ENUM ('instant', 'deferred');
CREATE TYPE job_state AS ENUM ('created', 'queued', 'assigned', 'running', 'succeeded', 'failed', 'canceled');
CREATE TYPE job_outcome AS ENUM ('success', 'failed', 'canceled');
CREATE TYPE event_name AS ENUM (
    'job_created',
    'job_queued',
    'job_assigned',
    'job_started',
    'job_succeeded',
    'job_failed',
    'job_canceled'
);

-- Clients
CREATE TABLE clients (
    id UUID PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- API keys
CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    client_id UUID NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL,
    key_prefix TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX api_keys_client_id_idx ON api_keys(client_id);
CREATE INDEX api_keys_key_prefix_idx ON api_keys(key_prefix);

-- Jobs
CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    client_id UUID NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    job_type job_type NOT NULL,
    state job_state NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 0,
    outcome job_outcome,
    outcome_reason TEXT,
    executed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    callback_url TEXT,
    work_kind TEXT NOT NULL
);
CREATE INDEX jobs_client_id_idx ON jobs(client_id);
CREATE INDEX jobs_state_idx ON jobs(state);
CREATE INDEX jobs_created_at_idx ON jobs(created_at);

-- Events
CREATE TABLE events (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    event_name event_name NOT NULL,
    prev_state job_state NOT NULL,
    next_state job_state NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX events_job_id_idx ON events(job_id);
CREATE INDEX events_occurred_at_idx ON events(occurred_at);

-- Reports
CREATE TABLE reports (
    job_id UUID PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    outcome job_outcome NOT NULL,
    outcome_reason TEXT,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NOT NULL,
    duration_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Idempotency keys
CREATE TABLE idempotency_keys (
    client_id UUID NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    job_id UUID REFERENCES jobs(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (client_id, idempotency_key)
);
