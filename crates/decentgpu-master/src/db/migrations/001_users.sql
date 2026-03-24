-- Migration 001: users and auth
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('hirer', 'worker', 'both')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
