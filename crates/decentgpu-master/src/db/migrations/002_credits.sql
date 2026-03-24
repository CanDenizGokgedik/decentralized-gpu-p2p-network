-- Migration 002: credit balances and transactions
CREATE TABLE IF NOT EXISTS credit_balances (
    user_id UUID PRIMARY KEY REFERENCES users(id),
    balance BIGINT NOT NULL DEFAULT 0 CHECK (balance >= 0),
    reserved BIGINT NOT NULL DEFAULT 0 CHECK (reserved >= 0)
);

CREATE TABLE IF NOT EXISTS credit_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    amount BIGINT NOT NULL,
    tx_type TEXT NOT NULL CHECK (tx_type IN ('deposit','job_charge','job_earn','refund')),
    job_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
