-- Webhooks and delivery tracking.

CREATE TYPE webhook_delivery_status AS ENUM ('pending', 'delivered', 'failed');

CREATE TABLE webhooks (
    id UUID PRIMARY KEY,
    client_id UUID NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    events TEXT[] NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX webhooks_client_url_idx ON webhooks(client_id, url);
CREATE UNIQUE INDEX webhooks_default_per_client_idx
    ON webhooks(client_id)
    WHERE is_default = true;
CREATE INDEX webhooks_events_gin_idx ON webhooks USING GIN(events);

CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY,
    webhook_id UUID REFERENCES webhooks(id) ON DELETE CASCADE,
    target_url TEXT NOT NULL,
    event_id UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    event_name event_name NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 0,
    status webhook_delivery_status NOT NULL DEFAULT 'pending',
    last_error TEXT,
    response_status INTEGER,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX webhook_deliveries_webhook_event_idx
    ON webhook_deliveries(webhook_id, event_id)
    WHERE webhook_id IS NOT NULL;
CREATE INDEX webhook_deliveries_status_next_idx
    ON webhook_deliveries(status, next_attempt_at);
