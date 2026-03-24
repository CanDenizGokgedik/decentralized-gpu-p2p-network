-- Migration 003: worker nodes
CREATE TABLE IF NOT EXISTS workers (
    peer_id TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    capabilities JSONB NOT NULL,
    uptime_score DOUBLE PRECISION NOT NULL DEFAULT 100.0,
    jobs_completed BIGINT NOT NULL DEFAULT 0,
    is_online BOOLEAN NOT NULL DEFAULT false,
    last_seen TIMESTAMPTZ,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS worker_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    peer_id TEXT NOT NULL REFERENCES workers(peer_id),
    connected_at TIMESTAMPTZ NOT NULL,
    disconnected_at TIMESTAMPTZ,
    disconnect_reason TEXT
);
