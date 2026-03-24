//! Worker list and lookup endpoints.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use super::{auth::ApiError, auth::AuthUser, AppState};
use crate::db::workers::WorkerRow;

// ─── Query params ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WorkerListQuery {
    /// Filter by GPU backend (e.g. "cuda", "rocm", "metal", "cpu_only").
    pub backend:     Option<String>,
    /// Minimum VRAM in MiB.
    pub min_vram_mb: Option<i64>,
    /// Return only online workers.
    pub online_only: Option<bool>,
    pub limit:       Option<i64>,
    pub offset:      Option<i64>,
}

// ─── Response DTO ─────────────────────────────────────────────────────────────

/// Worker summary returned by the API.
#[derive(Debug, Serialize)]
pub struct WorkerSummary {
    pub peer_id:        String,
    pub is_online:      bool,
    pub uptime_score:   f64,
    pub jobs_completed: i64,
    pub capabilities:   serde_json::Value,
    pub last_seen:      Option<chrono::DateTime<chrono::Utc>>,
}

impl From<WorkerRow> for WorkerSummary {
    fn from(r: WorkerRow) -> Self {
        Self {
            peer_id:        r.peer_id,
            is_online:      r.is_online,
            uptime_score:   r.uptime_score,
            jobs_completed: r.jobs_completed,
            capabilities:   r.capabilities,
            last_seen:      r.last_seen,
        }
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/workers` — list workers with optional filters.
pub async fn list_workers(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Query(q): Query<WorkerListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit       = q.limit.unwrap_or(50).clamp(1, 200);
    let offset      = q.offset.unwrap_or(0).max(0);
    let online_only = q.online_only.unwrap_or(false);

    let rows = state
        .db
        .workers
        .list_with_filters(q.backend.as_deref(), q.min_vram_mb, online_only, limit, offset)
        .await?;

    let resp: Vec<WorkerSummary> = rows.into_iter().map(Into::into).collect();
    Ok(Json(resp))
}

/// `GET /api/v1/workers/me` — return the authenticated user's own worker node.
pub async fn get_my_worker(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<impl IntoResponse, ApiError> {
    let row = state
        .db
        .workers
        .find_by_user_id(user.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("no worker registered for this account"))?;
    Ok(Json(WorkerSummary::from(row)))
}

/// `GET /api/v1/workers/:peer_id` — look up a specific worker by peer ID.
pub async fn get_worker_by_peer_id(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Path(peer_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let row = state
        .db
        .workers
        .find_by_peer_id(&peer_id)
        .await?
        .ok_or_else(|| ApiError::not_found("worker not found"))?;
    Ok(Json(WorkerSummary::from(row)))
}
