//! Database queries for the `workers` and `worker_sessions` tables.

use chrono::{DateTime, Utc};
use decentgpu_common::DecentGpuError;
use sqlx::PgPool;
use uuid::Uuid;

type Result<T> = std::result::Result<T, DecentGpuError>;

/// A row from the `workers` table.
#[derive(Debug, Clone)]
pub struct WorkerRow {
    pub peer_id:        String,
    pub user_id:        Uuid,
    pub capabilities:   serde_json::Value,
    pub uptime_score:   f64,
    pub jobs_completed: i64,
    pub is_online:      bool,
    pub last_seen:      Option<DateTime<Utc>>,
    pub registered_at:  DateTime<Utc>,
}

/// Repository for worker node operations.
#[derive(Clone)]
pub struct WorkerRepository {
    pool: PgPool,
}

impl WorkerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert or update a worker record on connect / reconnect.
    pub async fn upsert(
        &self,
        peer_id:      &str,
        user_id:      Uuid,
        capabilities: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO workers (peer_id, user_id, capabilities, is_online, last_seen)
               VALUES ($1, $2, $3, true, now())
               ON CONFLICT (peer_id) DO UPDATE
                 SET capabilities = EXCLUDED.capabilities,
                     is_online = true,
                     last_seen = now()"#,
            peer_id,
            user_id,
            capabilities,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Mark a worker as online or offline.
    pub async fn set_online(&self, peer_id: &str, online: bool) -> Result<()> {
        sqlx::query!(
            r#"UPDATE workers
               SET is_online = $2,
                   last_seen = CASE WHEN $2 THEN now() ELSE last_seen END
               WHERE peer_id = $1"#,
            peer_id,
            online,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Mark every worker offline (called at master startup).
    pub async fn mark_all_offline(&self) -> Result<u64> {
        let result = sqlx::query!("UPDATE workers SET is_online = false")
            .execute(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(result.rows_affected())
    }

    /// Update the uptime score for a worker (computed from session history).
    pub async fn update_uptime(&self, peer_id: &str, uptime_score: f64) -> Result<()> {
        sqlx::query!(
            "UPDATE workers SET uptime_score = $1 WHERE peer_id = $2",
            uptime_score,
            peer_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Increment the jobs_completed counter after a successful job.
    pub async fn increment_jobs_completed(&self, peer_id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE workers SET jobs_completed = jobs_completed + 1 WHERE peer_id = $1",
            peer_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// List all online workers ordered by uptime score descending.
    pub async fn list_online(&self) -> Result<Vec<WorkerRow>> {
        let rows = sqlx::query!(
            r#"SELECT peer_id, user_id, capabilities,
                      uptime_score, jobs_completed, is_online,
                      last_seen, registered_at
               FROM workers
               WHERE is_online = true
               ORDER BY uptime_score DESC, jobs_completed DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| WorkerRow {
                peer_id:        r.peer_id,
                user_id:        r.user_id,
                capabilities:   r.capabilities,
                uptime_score:   r.uptime_score,
                jobs_completed: r.jobs_completed,
                is_online:      r.is_online,
                last_seen:      r.last_seen,
                registered_at:  r.registered_at,
            })
            .collect())
    }

    /// List all workers (online and offline), ordered by online status then score.
    pub async fn list_all(&self) -> Result<Vec<WorkerRow>> {
        let rows = sqlx::query!(
            r#"SELECT peer_id, user_id, capabilities,
                      uptime_score, jobs_completed, is_online,
                      last_seen, registered_at
               FROM workers
               ORDER BY is_online DESC, uptime_score DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| WorkerRow {
                peer_id:        r.peer_id,
                user_id:        r.user_id,
                capabilities:   r.capabilities,
                uptime_score:   r.uptime_score,
                jobs_completed: r.jobs_completed,
                is_online:      r.is_online,
                last_seen:      r.last_seen,
                registered_at:  r.registered_at,
            })
            .collect())
    }

    /// Count currently online workers.
    pub async fn count_online(&self) -> Result<i64> {
        let row = sqlx::query!("SELECT COUNT(*) as cnt FROM workers WHERE is_online = true")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(row.cnt.unwrap_or(0))
    }

    /// Record the start of a worker session.
    pub async fn record_session_start(&self, peer_id: &str) -> Result<Uuid> {
        let row = sqlx::query!(
            r#"INSERT INTO worker_sessions (peer_id, connected_at)
               VALUES ($1, now())
               RETURNING id"#,
            peer_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(row.id)
    }

    /// Record the end of a worker session.
    pub async fn record_session_end(
        &self,
        session_id: Uuid,
        reason: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE worker_sessions
               SET disconnected_at = now(), disconnect_reason = $1
               WHERE id = $2"#,
            reason,
            session_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Calculate uptime score from session history (last 30 days).
    pub async fn calculate_uptime_score(&self, peer_id: &str) -> Result<f64> {
        let row = sqlx::query!(
            r#"SELECT
                 COALESCE(
                   SUM(
                     EXTRACT(EPOCH FROM (
                       COALESCE(disconnected_at, now()) - connected_at
                     ))
                   ), 0
                 )::float8 AS connected_seconds
               FROM worker_sessions
               WHERE peer_id = $1
                 AND connected_at > now() - INTERVAL '30 days'"#,
            peer_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        let window = 30.0 * 24.0 * 3600.0_f64;
        let connected = row.connected_seconds.unwrap_or(0.0);
        Ok((connected / window * 100.0).min(100.0))
    }

    /// Find a single worker by peer_id.
    pub async fn find_by_peer_id(&self, peer_id: &str) -> Result<Option<WorkerRow>> {
        let result = sqlx::query(
            "SELECT peer_id, user_id, capabilities, uptime_score, jobs_completed, is_online, last_seen, registered_at FROM workers WHERE peer_id = $1"
        )
        .bind(peer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(result.map(|r| map_worker_row(&r)))
    }

    /// Find a worker by user_id (most recently registered).
    pub async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<WorkerRow>> {
        let result = sqlx::query(
            "SELECT peer_id, user_id, capabilities, uptime_score, jobs_completed, is_online, last_seen, registered_at FROM workers WHERE user_id = $1 ORDER BY registered_at DESC LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(result.map(|r| map_worker_row(&r)))
    }

    /// Count total workers.
    pub async fn count_total(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*)::bigint AS cnt FROM workers")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        use sqlx::Row;
        Ok(row.get::<i64, _>("cnt"))
    }

    /// List workers with optional filters (backend, min_vram_mb, online_only).
    pub async fn list_with_filters(
        &self,
        backend:     Option<&str>,
        min_vram_mb: Option<i64>,
        online_only: bool,
        limit:       i64,
        offset:      i64,
    ) -> Result<Vec<WorkerRow>> {
        let mut sql = String::from(
            "SELECT peer_id, user_id, capabilities, uptime_score, jobs_completed, is_online, last_seen, registered_at FROM workers WHERE true"
        );
        if online_only {
            sql.push_str(" AND is_online = true");
        }
        if let Some(b) = backend {
            let cap = if b.is_empty() {
                String::new()
            } else {
                b[..1].to_uppercase() + &b[1..]
            };
            sql.push_str(&format!(
                " AND capabilities->'gpus' @> '[{{\"backend\":\"{cap}\"}}]'::jsonb"
            ));
        }
        if let Some(mb) = min_vram_mb {
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM jsonb_array_elements(capabilities->'gpus') g WHERE (g->>'vram_mb')::bigint >= {mb})"
            ));
        }
        sql.push_str(&format!(
            " ORDER BY is_online DESC, uptime_score DESC LIMIT {limit} OFFSET {offset}"
        ));

        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(rows.iter().map(map_worker_row).collect())
    }
}

fn map_worker_row(r: &sqlx::postgres::PgRow) -> WorkerRow {
    use sqlx::Row;
    WorkerRow {
        peer_id:        r.get("peer_id"),
        user_id:        r.get("user_id"),
        capabilities:   r.get("capabilities"),
        uptime_score:   r.get("uptime_score"),
        jobs_completed: r.get("jobs_completed"),
        is_online:      r.get("is_online"),
        last_seen:      r.get("last_seen"),
        registered_at:  r.get("registered_at"),
    }
}
