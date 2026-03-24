-- Migration 004: jobs and related tables
CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hirer_id UUID NOT NULL REFERENCES users(id),
    worker_peer_id TEXT REFERENCES workers(peer_id),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','assigned','running','completed','failed','cancelled')),
    gpu_backend TEXT NOT NULL,
    memory_limit_mb BIGINT,
    max_duration_secs BIGINT,
    credit_price BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    assigned_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_message TEXT,
    result_path TEXT
);

CREATE TABLE IF NOT EXISTS job_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES jobs(id),
    file_type TEXT NOT NULL CHECK (file_type IN ('code','requirements','result')),
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    sha256 TEXT NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS job_logs (
    id BIGSERIAL PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id),
    timestamp_ms BIGINT NOT NULL,
    level TEXT NOT NULL,
    message TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS job_logs_job_id_ts ON job_logs(job_id, timestamp_ms);
