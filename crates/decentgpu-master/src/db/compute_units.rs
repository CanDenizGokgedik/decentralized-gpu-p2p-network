//! Database queries for `compute_unit_balances` and `compute_unit_transactions`.
//!
//! Compute Units (CU) are a dimensionless measure of computational work.
//! 1 CU = 1 hour of baseline CPU compute.  Not a currency.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// A row from `compute_unit_balances`.
#[derive(Debug, Clone)]
pub struct BalanceRow {
    pub cu_balance:  i64,
    pub cu_reserved: i64,
}

/// A row from `compute_unit_transactions`.
#[derive(Debug, Clone, Serialize)]
pub struct TransactionRow {
    pub id:         Uuid,
    pub cu_amount:  i64,
    pub tx_type:    String,
    pub job_id:     Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Repository for compute-unit balance operations.
#[derive(Clone)]
pub struct ComputeUnitRepository {
    pool: PgPool,
}

impl ComputeUnitRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialise a zero balance for a new user (no-op if already exists).
    pub async fn init_balance(&self, user_id: Uuid) -> Result<()> {
        sqlx::query!(
            "INSERT INTO compute_unit_balances (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
            user_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch the current balance row for a user.
    pub async fn get_balance(&self, user_id: Uuid) -> Result<BalanceRow> {
        let row = sqlx::query!(
            "SELECT cu_balance, cu_reserved FROM compute_unit_balances WHERE user_id = $1",
            user_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(BalanceRow {
            cu_balance:  row.cu_balance,
            cu_reserved: row.cu_reserved,
        })
    }

    /// List the 50 most recent transactions for a user.
    pub async fn list_transactions(&self, user_id: Uuid) -> Result<Vec<TransactionRow>> {
        let rows = sqlx::query!(
            r#"SELECT id, cu_amount, tx_type, job_id, created_at
               FROM compute_unit_transactions
               WHERE user_id = $1
               ORDER BY created_at DESC
               LIMIT 50"#,
            user_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| TransactionRow {
                id:         r.id,
                cu_amount:  r.cu_amount,
                tx_type:    r.tx_type,
                job_id:     r.job_id,
                created_at: r.created_at,
            })
            .collect())
    }

    /// List transactions with pagination and optional type filter.
    /// Returns the page of rows and the total matching count.
    pub async fn list_transactions_paginated(
        &self,
        user_id: Uuid,
        limit:   i64,
        offset:  i64,
        tx_type: Option<&str>,
    ) -> Result<(Vec<TransactionRow>, i64)> {
        use sqlx::Row;

        let type_filter = tx_type
            .map(|t| format!(" AND tx_type = '{t}'"))
            .unwrap_or_default();

        let count_sql = format!(
            "SELECT COUNT(*)::bigint FROM compute_unit_transactions WHERE user_id = $1{type_filter}"
        );
        let count_row = sqlx::query(&count_sql)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
        let total: i64 = count_row.get(0);

        let data_sql = format!(
            "SELECT id, cu_amount, tx_type, job_id, created_at FROM compute_unit_transactions WHERE user_id = $1{type_filter} ORDER BY created_at DESC LIMIT {limit} OFFSET {offset}"
        );
        let rows = sqlx::query(&data_sql)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

        let txns = rows.iter().map(|r| TransactionRow {
            id:         r.get("id"),
            cu_amount:  r.get("cu_amount"),
            tx_type:    r.get("tx_type"),
            job_id:     r.get("job_id"),
            created_at: r.get("created_at"),
        }).collect();
        Ok((txns, total))
    }

    /// Sum of all allocation transactions (for admin stats).
    pub async fn sum_allocated(&self) -> Result<i64> {
        use sqlx::Row;
        let row = sqlx::query(
            "SELECT COALESCE(SUM(cu_amount), 0)::bigint AS s FROM compute_unit_transactions WHERE tx_type = 'allocation' AND cu_amount > 0"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("s"))
    }

    /// Sum of all job_debit transactions (CU consumed by jobs).
    pub async fn sum_consumed(&self) -> Result<i64> {
        use sqlx::Row;
        let row = sqlx::query(
            "SELECT COALESCE(SUM(ABS(cu_amount)), 0)::bigint AS s FROM compute_unit_transactions WHERE tx_type = 'job_debit'"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("s"))
    }
}
