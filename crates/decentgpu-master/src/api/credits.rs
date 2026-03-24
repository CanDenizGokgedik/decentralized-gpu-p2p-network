//! Compute Unit balance and transaction endpoints.

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    auth::{AdminUser, ApiError, AuthUser},
    AppState,
};
use crate::credits::ComputeUnitLedger;
use decentgpu_common::types::GpuBackend;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

/// Query parameters for the transactions endpoint.
#[derive(Debug, Deserialize)]
pub struct TransactionQuery {
    pub limit:   Option<i64>,
    pub offset:  Option<i64>,
    pub tx_type: Option<String>,
}

/// Admin-only allocation request body.
#[derive(Debug, Deserialize)]
pub struct AllocateRequest {
    /// Target user to receive the CUs.
    pub user_id: Uuid,
    /// Amount of CUs to allocate (must be 1–1 000 000).
    pub amount:  i64,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/compute-units/pricing` — public pricing table (no auth required).
pub async fn get_pricing() -> impl IntoResponse {
    Json(serde_json::json!({
        "base_rate_per_hour": decentgpu_common::types::pricing::BASE_RATE_PER_HOUR,
        "multipliers": {
            "cpu_only": 1.0,
            "metal":    3.0,
            "rocm":     4.0,
            "cuda":     5.0,
        },
        "example_prices_1h": {
            "cpu_only": ComputeUnitLedger::calculate_price(GpuBackend::CpuOnly, 1.0),
            "metal":    ComputeUnitLedger::calculate_price(GpuBackend::Metal,   1.0),
            "rocm":     ComputeUnitLedger::calculate_price(GpuBackend::Rocm,    1.0),
            "cuda":     ComputeUnitLedger::calculate_price(GpuBackend::Cuda,    1.0),
        },
        "unit": "CU",
    }))
}

/// `GET /api/v1/compute-units/balance` — current balance + last 10 transactions.
pub async fn get_balance(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<impl IntoResponse, ApiError> {
    let row = state.ledger.get_balance(user.user_id).await?;

    let (recent, _) = state
        .db
        .compute_units
        .list_transactions_paginated(user.user_id, 10, 0, None)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "cu_balance":            row.cu_balance,
        "cu_reserved":           row.cu_reserved,
        "cu_available":          row.cu_balance - row.cu_reserved,
        "unit":                  "CU",
        "recent_transactions":   recent,
    })))
}

/// `GET /api/v1/compute-units/transactions` — paginated transaction history.
pub async fn list_transactions(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(q): Query<TransactionQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit  = q.limit.unwrap_or(25).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let (txns, total) = state
        .db
        .compute_units
        .list_transactions_paginated(user.user_id, limit, offset, q.tx_type.as_deref())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "transactions": txns,
        "total":        total,
        "limit":        limit,
        "offset":       offset,
    })))
}

/// `POST /api/v1/compute-units/allocate` — add CUs to a user (admin only).
pub async fn allocate(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<AllocateRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.amount < 1 || body.amount > 1_000_000 {
        return Err(ApiError::bad_request("amount must be between 1 and 1,000,000"));
    }

    let new_balance = state.ledger.allocate(body.user_id, body.amount).await?;

    Ok(Json(serde_json::json!({
        "user_id":    body.user_id,
        "allocated":  body.amount,
        "cu_balance": new_balance,
    })))
}
