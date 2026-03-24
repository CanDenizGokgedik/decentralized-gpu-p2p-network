//! Database queries for the `jobs`, `job_files`, and `job_logs` tables.

use chrono::{DateTime, Utc};
use decentgpu_common::DecentGpuError;
use sqlx::PgPool;
use uuid::Uuid;

type Result<T> = std::result::Result<T, DecentGpuError>;

/// A row from the `jobs` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobRow {
    pub id:                Uuid,
    pub hirer_id:          Uuid,
    pub worker_peer_id:    Option<String>,
    pub status:            String,
    pub gpu_backend:       String,
    pub memory_limit_mb:   Option<i64>,
    pub max_duration_secs: Option<i64>,
    /// Compute Unit price for this job.
    pub cu_price:          Option<i64>,
    pub created_at:        DateTime<Utc>,
    pub assigned_at:       Option<DateTime<Utc>>,
    pub started_at:        Option<DateTime<Utc>>,
    pub finished_at:       Option<DateTime<Utc>>,
    pub error_message:     Option<String>,
    pub result_path:       Option<String>,
}

/// Repository for job lifecycle operations.
#[derive(Clone)]
pub struct JobRepository {
    pool: PgPool,
}

impl JobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new job in `pending` status.
    pub async fn create(
        &self,
        hirer_id:          Uuid,
        gpu_backend:       &str,
        memory_limit_mb:   Option<i64>,
        max_duration_secs: Option<i64>,
        cu_price:          Option<i64>,
    ) -> Result<JobRow> {
        sqlx::query_as!(
            JobRow,
            r#"INSERT INTO jobs
                 (hirer_id, gpu_backend, memory_limit_mb, max_duration_secs, cu_price)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING
                 id, hirer_id, worker_peer_id, status, gpu_backend,
                 memory_limit_mb, max_duration_secs, cu_price,
                 created_at, assigned_at, started_at, finished_at,
                 error_message, result_path"#,
            hirer_id,
            gpu_backend,
            memory_limit_mb,
            max_duration_secs,
            cu_price,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// Atomically transition a job's status (guarded by `from_status`).
    ///
    /// Returns `Conflict` if the job is not currently in `from_status`.
    pub async fn transition_status(
        &self,
        job_id:         Uuid,
        from_status:    &str,
        to_status:      &str,
        worker_peer_id: Option<&str>,
        error:          Option<&str>,
        result_path:    Option<&str>,
    ) -> Result<JobRow> {
        sqlx::query_as!(
            JobRow,
            r#"UPDATE jobs SET
                 status         = $1,
                 worker_peer_id = COALESCE($2, worker_peer_id),
                 error_message  = COALESCE($3, error_message),
                 result_path    = COALESCE($4, result_path),
                 assigned_at = CASE WHEN $1 = 'assigned'  THEN now() ELSE assigned_at END,
                 started_at  = CASE WHEN $1 = 'running'   THEN now() ELSE started_at  END,
                 finished_at = CASE WHEN $1 IN ('completed','failed','cancelled')
                                    THEN now() ELSE finished_at END
               WHERE id = $5 AND status = $6
               RETURNING
                 id, hirer_id, worker_peer_id, status, gpu_backend,
                 memory_limit_mb, max_duration_secs, cu_price,
                 created_at, assigned_at, started_at, finished_at,
                 error_message, result_path"#,
            to_status,
            worker_peer_id,
            error,
            result_path,
            job_id,
            from_status,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?
        .ok_or_else(|| {
            DecentGpuError::Conflict(format!(
                "job {job_id} not in status '{from_status}'"
            ))
        })
    }

    /// Fetch a single job by ID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<JobRow>> {
        sqlx::query_as!(
            JobRow,
            r#"SELECT id, hirer_id, worker_peer_id, status, gpu_backend,
                      memory_limit_mb, max_duration_secs, cu_price,
                      created_at, assigned_at, started_at, finished_at,
                      error_message, result_path
               FROM jobs WHERE id = $1"#,
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// List jobs for a hirer, newest first, with pagination.
    pub async fn list_by_hirer(
        &self,
        hirer_id: Uuid,
        limit:    i64,
        offset:   i64,
    ) -> Result<Vec<JobRow>> {
        sqlx::query_as!(
            JobRow,
            r#"SELECT id, hirer_id, worker_peer_id, status, gpu_backend,
                      memory_limit_mb, max_duration_secs, cu_price,
                      created_at, assigned_at, started_at, finished_at,
                      error_message, result_path
               FROM jobs
               WHERE hirer_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
            hirer_id,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// Append a log line for a job.
    pub async fn append_log(
        &self,
        job_id:       Uuid,
        timestamp_ms: i64,
        level:        &str,
        message:      &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO job_logs (job_id, timestamp_ms, level, message)
               VALUES ($1, $2, $3, $4)"#,
            job_id,
            timestamp_ms,
            level,
            message,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// Fetch log lines for a job since a given timestamp (for SSE replay).
    pub async fn get_logs_since(
        &self,
        job_id:   Uuid,
        since_ms: i64,
        limit:    i64,
    ) -> Result<Vec<(i64, String, String)>> {
        let rows = sqlx::query!(
            r#"SELECT timestamp_ms, level, message
               FROM job_logs
               WHERE job_id = $1 AND timestamp_ms > $2
               ORDER BY timestamp_ms ASC
               LIMIT $3"#,
            job_id,
            since_ms,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| (r.timestamp_ms, r.level, r.message))
            .collect())
    }

    /// Record an uploaded file associated with a job.
    pub async fn insert_file(
        &self,
        job_id:    Uuid,
        file_type: &str,
        file_path: &str,
        file_size: i64,
        sha256:    &str,
    ) -> Result<Uuid> {
        let row = sqlx::query!(
            r#"INSERT INTO job_files (job_id, file_type, file_path, file_size, sha256)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id"#,
            job_id,
            file_type,
            file_path,
            file_size,
            sha256,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(row.id)
    }

    /// Update the cu_price for a job.
    pub async fn update_cu_price(&self, job_id: Uuid, cu_price: i64) -> Result<()> {
        sqlx::query("UPDATE jobs SET cu_price = $1 WHERE id = $2")
            .bind(cu_price)
            .bind(job_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(())
    }

    /// List all jobs (admin view) with optional status filter and pagination.
    pub async fn list_all(
        &self,
        status: Option<&str>,
        limit:  i64,
        offset: i64,
    ) -> Result<Vec<JobRow>> {
        let mut sql = String::from(
            "SELECT id, hirer_id, worker_peer_id, status, gpu_backend, memory_limit_mb, max_duration_secs, cu_price, created_at, assigned_at, started_at, finished_at, error_message, result_path FROM jobs WHERE true"
        );
        if let Some(s) = status {
            sql.push_str(&format!(" AND status = '{s}'"));
        }
        sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT {limit} OFFSET {offset}"
        ));

        sqlx::query_as::<_, JobRow>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// Count all jobs with optional status filter.
    pub async fn count_all(&self, status: Option<&str>) -> Result<i64> {
        let sql = if let Some(s) = status {
            format!("SELECT COUNT(*)::bigint AS cnt FROM jobs WHERE status = '{s}'")
        } else {
            "SELECT COUNT(*)::bigint AS cnt FROM jobs".to_string()
        };
        let row = sqlx::query(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        use sqlx::Row;
        Ok(row.get::<i64, _>("cnt"))
    }

    /// Count jobs created today.
    pub async fn count_today(&self) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*)::bigint AS cnt FROM jobs WHERE created_at >= CURRENT_DATE"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        use sqlx::Row;
        Ok(row.get::<i64, _>("cnt"))
    }
}
