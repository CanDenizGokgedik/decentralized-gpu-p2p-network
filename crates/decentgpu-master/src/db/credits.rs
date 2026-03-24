//! Database queries for `credit_balances` and `credit_transactions` tables.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// A row from `credit_balances`.
pub struct BalanceRow {
    pub balance: i64,
    pub reserved: i64,
}

/// A row from `credit_transactions`.
#[derive(Debug, Serialize)]
pub struct TransactionRow {
    pub id: Uuid,
    pub amount: i64,
    pub tx_type: String,
    pub job_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Initialise a zero balance for a new user (no-op if already exists).
pub async fn init_balance(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query!(
        "INSERT INTO credit_balances (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
        user_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch the current balance for a user.
pub async fn get_balance(pool: &PgPool, user_id: Uuid) -> Result<BalanceRow> {
    let row = sqlx::query!(
        "SELECT balance, reserved FROM credit_balances WHERE user_id = $1",
        user_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(BalanceRow {
        balance: row.balance,
        reserved: row.reserved,
    })
}

/// List the 50 most recent transactions for a user.
pub async fn list_transactions(pool: &PgPool, user_id: Uuid) -> Result<Vec<TransactionRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, amount, tx_type, job_id, created_at
           FROM credit_transactions
           WHERE user_id = $1
           ORDER BY created_at DESC
           LIMIT 50"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TransactionRow {
            id: r.id,
            amount: r.amount,
            tx_type: r.tx_type,
            job_id: r.job_id,
            created_at: r.created_at,
        })
        .collect())
}
