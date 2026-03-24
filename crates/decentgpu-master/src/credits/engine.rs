//! Atomic Compute Unit ledger — all balance mutations happen here.
//!
//! Every method runs inside a single PostgreSQL transaction using
//! `SELECT ... FOR UPDATE` to prevent race conditions.
//!
//! Compute Units (CU) are a dimensionless measure of work; 1 CU = 1 hour of
//! baseline CPU compute. They are NOT a currency.

use decentgpu_common::{
    types::{pricing, GpuBackend},
    DecentGpuError,
};
use sqlx::PgPool;
use uuid::Uuid;

type Result<T> = std::result::Result<T, DecentGpuError>;

/// Reusable balance row returned by [`ComputeUnitLedger::get_balance`].
#[derive(Debug, Clone)]
pub struct BalanceRow {
    pub cu_balance:  i64,
    pub cu_reserved: i64,
}

/// Atomic Compute Unit ledger.
#[derive(Clone)]
pub struct ComputeUnitLedger {
    pool: PgPool,
}

impl ComputeUnitLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Pure helpers ──────────────────────────────────────────────────────────

    /// Calculate the total CU price for a job (pure — no I/O).
    ///
    /// `duration_hours` is the *maximum* duration used for upfront reservation.
    pub fn calculate_price(backend: GpuBackend, duration_hours: f64) -> i64 {
        let rate = pricing::BASE_RATE_PER_HOUR as f64
            * duration_hours
            * backend.price_multiplier();
        rate.ceil() as i64
    }

    // ── Write operations ──────────────────────────────────────────────────────

    /// Initialise a zero CU balance for a new user (no-op if already exists).
    pub async fn initialize(&self, user_id: Uuid) -> Result<()> {
        sqlx::query!(
            "INSERT INTO compute_unit_balances (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
            user_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Allocate (add) CUs to a user's balance (admin/test allocation).
    ///
    /// Returns the new balance.
    pub async fn allocate(&self, user_id: Uuid, amount: i64) -> Result<i64> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        let row = sqlx::query!(
            "SELECT cu_balance, cu_reserved \
             FROM compute_unit_balances WHERE user_id = $1 FOR UPDATE",
            user_id,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?
        .ok_or_else(|| DecentGpuError::NotFound(format!("balance not found for user {user_id}")))?;

        let new_balance = row.cu_balance + amount;

        sqlx::query!(
            "UPDATE compute_unit_balances SET cu_balance = $2 WHERE user_id = $1",
            user_id,
            new_balance,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        sqlx::query!(
            "INSERT INTO compute_unit_transactions \
               (user_id, cu_amount, tx_type) VALUES ($1, $2, 'allocation')",
            user_id,
            amount,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        tx.commit().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(new_balance)
    }

    /// Reserve CUs for a job (moves balance → reserved).
    ///
    /// Returns `InsufficientComputeUnits` if available < amount.
    pub async fn reserve(
        &self,
        hirer_id: Uuid,
        job_id:   Uuid,
        amount:   i64,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        let row = sqlx::query!(
            "SELECT cu_balance, cu_reserved \
             FROM compute_unit_balances WHERE user_id = $1 FOR UPDATE",
            hirer_id,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?
        .ok_or_else(|| DecentGpuError::NotFound("balance not found".into()))?;

        let available = row.cu_balance - row.cu_reserved;
        if available < amount {
            return Err(DecentGpuError::InsufficientComputeUnits {
                available,
                required: amount,
            });
        }

        sqlx::query!(
            "UPDATE compute_unit_balances \
             SET cu_reserved = cu_reserved + $2 WHERE user_id = $1",
            hirer_id,
            amount,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        sqlx::query!(
            "INSERT INTO compute_unit_transactions \
               (user_id, cu_amount, tx_type, job_id) VALUES ($1, $2, 'job_debit', $3)",
            hirer_id,
            -amount,
            job_id,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        tx.commit().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Settle a completed job: charge hirer for actual usage, pay worker, refund excess.
    ///
    /// Lock order: hirer then worker (lower UUID first) to prevent deadlock.
    pub async fn settle(
        &self,
        hirer_id:             Uuid,
        worker_user_id:       Uuid,
        job_id:               Uuid,
        reserved_amount:      i64,
        actual_duration_secs: u64,
        backend:              GpuBackend,
    ) -> Result<()> {
        let actual_hours  = actual_duration_secs as f64 / 3600.0;
        let actual_charge = Self::calculate_price(backend, actual_hours).min(reserved_amount);
        let refund        = reserved_amount - actual_charge;
        let worker_payout = (actual_charge as f64 * pricing::WORKER_PAYOUT_FRACTION) as i64;

        let mut tx = self.pool.begin().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        // Lock in consistent UUID order to avoid deadlocks.
        let (first, second) = if hirer_id < worker_user_id {
            (hirer_id, worker_user_id)
        } else {
            (worker_user_id, hirer_id)
        };

        sqlx::query!(
            "SELECT cu_balance FROM compute_unit_balances \
             WHERE user_id = $1 FOR UPDATE",
            first,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        sqlx::query!(
            "SELECT cu_balance FROM compute_unit_balances \
             WHERE user_id = $1 FOR UPDATE",
            second,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        // Release reservation + apply refund for hirer.
        sqlx::query!(
            "UPDATE compute_unit_balances \
             SET cu_balance = cu_balance + $2, cu_reserved = cu_reserved - $3 \
             WHERE user_id = $1",
            hirer_id,
            refund,
            reserved_amount,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        // Pay worker.
        sqlx::query!(
            "UPDATE compute_unit_balances \
             SET cu_balance = cu_balance + $2 WHERE user_id = $1",
            worker_user_id,
            worker_payout,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        if refund > 0 {
            sqlx::query!(
                "INSERT INTO compute_unit_transactions \
                   (user_id, cu_amount, tx_type, job_id) VALUES ($1, $2, 'job_refund', $3)",
                hirer_id,
                refund,
                job_id,
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        }

        sqlx::query!(
            "INSERT INTO compute_unit_transactions \
               (user_id, cu_amount, tx_type, job_id) VALUES ($1, $2, 'job_credit', $3)",
            worker_user_id,
            worker_payout,
            job_id,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        tx.commit().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Refund the full reserved amount to the hirer (e.g. job failed / cancelled).
    pub async fn refund_full(
        &self,
        hirer_id:        Uuid,
        job_id:          Uuid,
        reserved_amount: i64,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        sqlx::query!(
            "UPDATE compute_unit_balances \
             SET cu_balance = cu_balance + $2, cu_reserved = cu_reserved - $2 \
             WHERE user_id = $1",
            hirer_id,
            reserved_amount,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        sqlx::query!(
            "INSERT INTO compute_unit_transactions \
               (user_id, cu_amount, tx_type, job_id) VALUES ($1, $2, 'job_refund', $3)",
            hirer_id,
            reserved_amount,
            job_id,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        tx.commit().await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    // ── Read operations ───────────────────────────────────────────────────────

    /// Fetch the current balance row for a user (non-locking read).
    pub async fn get_balance(&self, user_id: Uuid) -> Result<BalanceRow> {
        let row = sqlx::query!(
            "SELECT cu_balance, cu_reserved \
             FROM compute_unit_balances WHERE user_id = $1",
            user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?
        .ok_or_else(|| DecentGpuError::NotFound(format!("no balance for user {user_id}")))?;

        Ok(BalanceRow {
            cu_balance:  row.cu_balance,
            cu_reserved: row.cu_reserved,
        })
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use decentgpu_common::types::GpuBackend;

    /// `calculate_price` must be deterministic and honour the multipliers.
    #[test]
    fn calculate_price_cpu_only_1h() {
        let price = ComputeUnitLedger::calculate_price(GpuBackend::CpuOnly, 1.0);
        assert_eq!(price, pricing::BASE_RATE_PER_HOUR as i64);
    }

    #[test]
    fn calculate_price_cuda_2h() {
        let expected = (pricing::BASE_RATE_PER_HOUR as f64 * 2.0 * 5.0).ceil() as i64;
        let price = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 2.0);
        assert_eq!(price, expected);
    }

    #[test]
    fn calculate_price_metal_half_hour() {
        let expected = (pricing::BASE_RATE_PER_HOUR as f64 * 0.5 * 3.0).ceil() as i64;
        let price = ComputeUnitLedger::calculate_price(GpuBackend::Metal, 0.5);
        assert_eq!(price, expected);
    }

    #[test]
    fn calculate_price_rocm_1h() {
        let expected = (pricing::BASE_RATE_PER_HOUR as f64 * 1.0 * 4.0).ceil() as i64;
        let price = ComputeUnitLedger::calculate_price(GpuBackend::Rocm, 1.0);
        assert_eq!(price, expected);
    }
}
