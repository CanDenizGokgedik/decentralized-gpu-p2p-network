//! Admin-only API endpoints.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{auth::{AdminUser, ApiError}, AppState};

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub search: Option<String>,
    pub role:   Option<String>,
    pub limit:  Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/admin/users`
pub async fn list_users(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(q): Query<UserListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit  = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).max(0);

    let total = state.db.users.count_all(q.search.as_deref(), q.role.as_deref()).await?;
    let users = state.db.users.list_all(q.search.as_deref(), q.role.as_deref(), limit, offset).await?;

    let list: Vec<serde_json::Value> = users.iter().map(|u| serde_json::json!({
        "id":         u.id,
        "email":      u.email,
        "role":       u.role,
        "created_at": u.created_at,
    })).collect();

    Ok(Json(serde_json::json!({
        "users":  list,
        "total":  total,
        "limit":  limit,
        "offset": offset,
    })))
}

/// `GET /api/v1/admin/users/:id`
pub async fn get_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user = state
        .db
        .users
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    Ok(Json(serde_json::json!({
        "id":         user.id,
        "email":      user.email,
        "role":       user.role,
        "created_at": user.created_at,
    })))
}

/// `PATCH /api/v1/admin/users/:id/role`
pub async fn update_user_role(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, ApiError> {
    const VALID: &[&str] = &["hirer", "worker", "both", "admin"];
    if !VALID.contains(&body.role.as_str()) {
        return Err(ApiError::bad_request(format!(
            "invalid role; must be one of: {}",
            VALID.join(", ")
        )));
    }

    // Prevent self-demotion.
    if id == admin.user_id && body.role != "admin" {
        return Err(ApiError::bad_request("cannot demote yourself"));
    }

    // Guard: must not remove the last admin.
    if body.role != "admin" {
        let target = state
            .db
            .users
            .find_by_id(id)
            .await?
            .ok_or_else(|| ApiError::not_found("user not found"))?;

        if target.role == "admin" {
            let admin_count = state.db.users.count_admins().await?;
            if admin_count <= 1 {
                return Err(ApiError::bad_request("cannot demote the last admin"));
            }
        }
    }

    let updated = state.db.users.update_role(id, &body.role).await?;

    Ok(Json(serde_json::json!({
        "id":   updated.id,
        "role": updated.role,
    })))
}

/// `GET /api/v1/admin/stats`
pub async fn get_stats(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> impl IntoResponse {
    use sqlx::Row;

    let users_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db.pool)
        .await
        .unwrap_or(0);

    let workers_online: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workers WHERE is_online = true")
        .fetch_one(&state.db.pool)
        .await
        .unwrap_or(0);

    let jobs_running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE status = 'running'")
        .fetch_one(&state.db.pool)
        .await
        .unwrap_or(0);

    let jobs_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE status = 'completed' AND finished_at >= CURRENT_DATE"
    )
    .fetch_one(&state.db.pool)
    .await
    .unwrap_or(0);

    let jobs_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
        .fetch_one(&state.db.pool)
        .await
        .unwrap_or(0);

    let cu_allocated: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(cu_amount), 0) FROM compute_unit_transactions WHERE tx_type = 'allocation'"
    )
    .fetch_one(&state.db.pool)
    .await
    .unwrap_or(0);

    let cu_consumed: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(ABS(cu_amount)), 0) FROM compute_unit_transactions WHERE tx_type = 'job_debit'"
    )
    .fetch_one(&state.db.pool)
    .await
    .unwrap_or(0);

    // Jobs by day — last 14 days
    let jobs_by_day: Vec<serde_json::Value> = sqlx::query(
        "SELECT DATE(created_at)::text as day, COUNT(*)::bigint as count,
         COUNT(*) FILTER (WHERE status = 'completed')::bigint as completed,
         COUNT(*) FILTER (WHERE status = 'failed')::bigint as failed
         FROM jobs WHERE created_at >= NOW() - INTERVAL '14 days'
         GROUP BY DATE(created_at) ORDER BY day ASC"
    )
    .fetch_all(&state.db.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| {
        serde_json::json!({
            "day":       row.try_get::<String, _>("day").unwrap_or_default(),
            "count":     row.try_get::<i64, _>("count").unwrap_or(0),
            "completed": row.try_get::<i64, _>("completed").unwrap_or(0),
            "failed":    row.try_get::<i64, _>("failed").unwrap_or(0),
        })
    })
    .collect();

    // Jobs by backend
    let jobs_by_backend: Vec<serde_json::Value> = sqlx::query(
        "SELECT gpu_backend, COUNT(*)::bigint as count FROM jobs GROUP BY gpu_backend ORDER BY count DESC"
    )
    .fetch_all(&state.db.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| {
        serde_json::json!({
            "backend": row.try_get::<Option<String>, _>("gpu_backend").unwrap_or(None).unwrap_or_else(|| "cpu_only".to_string()),
            "count":   row.try_get::<i64, _>("count").unwrap_or(0),
        })
    })
    .collect();

    Json(serde_json::json!({
        "users_total":       users_total,
        "workers_online":    workers_online,
        "jobs_running":      jobs_running,
        "jobs_today":        jobs_today,
        "jobs_total":        jobs_total,
        "cu_allocated_total": cu_allocated,
        "cu_consumed_total":  cu_consumed,
        "jobs_by_day":       jobs_by_day,
        "jobs_by_backend":   jobs_by_backend,
    }))
}

/// `POST /api/v1/admin/workers/:peer_id/disconnect`
pub async fn disconnect_worker(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(peer_id_str): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let peer_id: libp2p::PeerId = peer_id_str
        .parse()
        .map_err(|_| ApiError::bad_request("invalid peer_id format"))?;

    // Mark offline in DB.
    state.db.workers.set_online(&peer_id_str, false).await?;

    // Tell the P2P layer to cancel any running job on that peer.
    state
        .p2p_cmd_tx
        .send(crate::p2p::JobCommand::Cancel {
            peer_id,
            job_id: String::new(), // empty = disconnect signal, not a specific job
        })
        .await
        .map_err(|_| ApiError::internal("P2P command channel closed"))?;

    Ok(Json(serde_json::json!({ "disconnected": peer_id_str })))
}
