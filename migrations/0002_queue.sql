-- Queue/worker lease fields for milestone 5.

ALTER TABLE jobs
    ADD COLUMN lease_owner TEXT,
    ADD COLUMN lease_expires_at TIMESTAMPTZ,
    ADD COLUMN heartbeat_at TIMESTAMPTZ;

CREATE INDEX jobs_lease_expires_at_idx ON jobs(lease_expires_at);
CREATE INDEX jobs_state_lease_idx ON jobs(state, lease_expires_at);
