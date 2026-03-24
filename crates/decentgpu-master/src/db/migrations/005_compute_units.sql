-- Migration 005: Rename credit tables/columns to compute_unit terminology.
--
-- Compute Units (CU) are a dimensionless measure of computational work.
-- 1 CU = 1 hour of baseline CPU compute.  This is NOT a currency.

-- ── Rename credit_balances → compute_unit_balances ────────────────────────
ALTER TABLE credit_balances RENAME TO compute_unit_balances;

ALTER TABLE compute_unit_balances
    RENAME COLUMN balance  TO cu_balance;

ALTER TABLE compute_unit_balances
    RENAME COLUMN reserved TO cu_reserved;

-- Update CHECK constraints (drop old, add new).
ALTER TABLE compute_unit_balances
    DROP CONSTRAINT IF EXISTS credit_balances_balance_check;
ALTER TABLE compute_unit_balances
    DROP CONSTRAINT IF EXISTS credit_balances_reserved_check;

ALTER TABLE compute_unit_balances
    ADD CONSTRAINT compute_unit_balances_cu_balance_check  CHECK (cu_balance  >= 0),
    ADD CONSTRAINT compute_unit_balances_cu_reserved_check CHECK (cu_reserved >= 0);

-- ── Rename credit_transactions → compute_unit_transactions ────────────────
ALTER TABLE credit_transactions RENAME TO compute_unit_transactions;

ALTER TABLE compute_unit_transactions
    RENAME COLUMN amount TO cu_amount;

-- Update tx_type CHECK constraint with new vocabulary.
ALTER TABLE compute_unit_transactions
    DROP CONSTRAINT IF EXISTS credit_transactions_tx_type_check;

ALTER TABLE compute_unit_transactions
    ADD CONSTRAINT compute_unit_transactions_tx_type_check
        CHECK (tx_type IN ('allocation','job_debit','job_credit','job_refund'));

-- Migrate existing tx_type values to new names.
UPDATE compute_unit_transactions SET tx_type = 'allocation'  WHERE tx_type = 'deposit';
UPDATE compute_unit_transactions SET tx_type = 'job_debit'   WHERE tx_type = 'job_charge';
UPDATE compute_unit_transactions SET tx_type = 'job_credit'  WHERE tx_type = 'job_earn';
UPDATE compute_unit_transactions SET tx_type = 'job_refund'  WHERE tx_type = 'refund';

-- ── Rename credit_price → cu_price in jobs ────────────────────────────────
ALTER TABLE jobs RENAME COLUMN credit_price TO cu_price;
