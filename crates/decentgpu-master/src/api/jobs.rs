//! Job CRUD and file upload endpoints.

use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

use super::{auth::ApiError, auth::AuthUser, AppState};
use crate::db::jobs::JobRow;
use crate::scheduler::MatchRequest;
use decentgpu_common::types::GpuBackend;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

/// Request body for job creation.
#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub gpu_backend:       String,
    pub memory_limit_mb:   Option<i64>,
    pub max_duration_secs: Option<i64>,
    /// Retained in the DTO for API compatibility but not persisted (scheduled at assignment).
    pub cpu_limit_percent: Option<i64>,
}

/// Public job view returned by the API.
#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub id:                Uuid,
    pub status:            String,
    pub gpu_backend:       String,
    pub memory_limit_mb:   Option<i64>,
    pub max_duration_secs: Option<i64>,
    pub cu_price:          Option<i64>,
    pub created_at:        chrono::DateTime<Utc>,
    pub worker_peer_id:    Option<String>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/jobs` — list jobs for the authenticated hirer.
pub async fn list_jobs(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<impl IntoResponse, ApiError> {
    let rows = state.db.jobs.list_by_hirer(user.user_id, 100, 0).await?;
    let resp: Vec<JobResponse> = rows.into_iter().map(Into::into).collect();
    Ok(Json(resp))
}

/// `GET /api/v1/jobs/:id` — get a single job.
pub async fn get_job(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let row = state
        .db
        .jobs
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("job not found"))?;

    if row.hirer_id != user.user_id {
        return Err(ApiError::forbidden("not your job"));
    }

    Ok(Json(JobResponse::from(row)))
}

/// `POST /api/v1/jobs` — create a new job (multipart/form-data).
///
/// Accepts:
/// - `gpu_backend`       — "cpu_only" | "cuda" | "metal" | "rocm"
/// - `memory_limit_mb`   — integer (optional, default 512)
/// - `max_duration_secs` — integer (optional, default 3600)
/// - `code`              — Python source file
/// - `requirements`      — requirements.txt (optional)
pub async fn create_job(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut gpu_backend:       Option<String> = None;
    let mut memory_limit_mb:   Option<i64>    = None;
    let mut max_duration_secs: Option<i64>    = None;
    let mut code_bytes:        Option<Bytes>  = None;
    let mut req_bytes:         Option<Bytes>  = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("multipart error: {e}")))?
    {
        match field.name().unwrap_or_default() {
            "gpu_backend" => {
                gpu_backend = Some(
                    field.text().await
                        .map_err(|e| ApiError::bad_request(format!("read field: {e}")))?,
                );
            }
            "memory_limit_mb" => {
                let t = field.text().await
                    .map_err(|e| ApiError::bad_request(format!("read field: {e}")))?;
                memory_limit_mb = t.parse().ok();
            }
            "max_duration_secs" => {
                let t = field.text().await
                    .map_err(|e| ApiError::bad_request(format!("read field: {e}")))?;
                max_duration_secs = t.parse().ok();
            }
            "code" => {
                code_bytes = Some(
                    field.bytes().await
                        .map_err(|e| ApiError::bad_request(format!("read code: {e}")))?,
                );
            }
            "requirements" => {
                req_bytes = Some(
                    field.bytes().await
                        .map_err(|e| ApiError::bad_request(format!("read requirements: {e}")))?,
                );
            }
            _ => { let _ = field.bytes().await; }
        }
    }

    let backend = gpu_backend.unwrap_or_else(|| "cpu_only".to_string());
    if !matches!(backend.as_str(), "cpu_only" | "cuda" | "metal" | "rocm") {
        return Err(ApiError::bad_request("unknown gpu_backend"));
    }
    let code = code_bytes.ok_or_else(|| ApiError::bad_request("missing 'code' field"))?;
    let requirements = req_bytes.unwrap_or_else(|| Bytes::from_static(b""));

    let mem_mb  = memory_limit_mb.unwrap_or(512);
    let max_dur = max_duration_secs.unwrap_or(3600);

    // Create the job record in DB.
    let row = state.db.jobs.create(
        user.user_id,
        &backend,
        Some(mem_mb),
        Some(max_dur),
        None, // cu_price set at assignment time
    ).await?;

    let job_id = row.id;
    tracing::info!(%job_id, %backend, "job created in DB, saving files");

    // Save code + requirements to the storage directory.
    let job_dir = PathBuf::from(state.storage_path.as_ref()).join(job_id.to_string());
    tokio::fs::create_dir_all(&job_dir).await
        .map_err(|e| ApiError::internal(format!("mkdir: {e}")))?;

    let code_path = job_dir.join("code.py");
    let req_path  = job_dir.join("requirements.txt");
    tokio::fs::write(&code_path, &code).await
        .map_err(|e| ApiError::internal(format!("write code: {e}")))?;
    tokio::fs::write(&req_path, &requirements).await
        .map_err(|e| ApiError::internal(format!("write requirements: {e}")))?;

    tracing::info!(%job_id, code_size = code.len(), "files saved");

    // Spawn async task: build Docker image → enqueue in scheduler.
    // The HTTP response returns immediately; build happens in the background.
    let state_clone   = state.clone();
    let backend_clone = backend.clone();
    tokio::spawn(async move {
        let required_backend: GpuBackend = backend_clone.parse()
            .unwrap_or(GpuBackend::CpuOnly);

        // Build Docker image (if builder is available).
        let image_result = if let Some(builder) = &state_clone.docker_builder {
            tracing::info!(%job_id, "starting Docker image build");
            builder.build_image(
                &job_id.to_string(),
                &code_path,
                &req_path,
                required_backend,
            ).await
        } else {
            // Docker unavailable — skip image build, run without container.
            tracing::warn!(%job_id, "Docker unavailable, skipping image build");
            Err(anyhow::anyhow!("Docker unavailable"))
        };

        match image_result {
            Ok(image_path) => {
                tracing::info!(%job_id, path = %image_path.display(), "Docker image built");

                // Compute image hash + size.
                let image_data = match tokio::fs::read(&image_path).await {
                    Ok(d)  => d,
                    Err(e) => {
                        tracing::error!(%job_id, err = %e, "failed to read image tar");
                        return;
                    }
                };
                let mut hasher = sha2::Sha256::new();
                hasher.update(&image_data);
                let _image_hash = hex::encode(hasher.finalize());
                let _image_size = image_data.len() as u64;

                // Enqueue for scheduling.
                state_clone.scheduler.enqueue(job_id, MatchRequest {
                    required_backend,
                    memory_limit_mb:   mem_mb as u64,
                    max_duration_secs: max_dur as u64,
                }).await;
                tracing::info!(%job_id, "job enqueued in scheduler after Docker build");
            }
            Err(e) => {
                tracing::warn!(%job_id, err = %e,
                    "Docker build failed/skipped — enqueuing anyway (worker will run code directly)");
                // Enqueue anyway so workers can attempt the job without a container.
                state_clone.scheduler.enqueue(job_id, MatchRequest {
                    required_backend,
                    memory_limit_mb:   mem_mb as u64,
                    max_duration_secs: max_dur as u64,
                }).await;
                tracing::info!(%job_id, "job enqueued without Docker image");
            }
        }
    });

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "job_id": job_id }))))
}

/// `POST /api/v1/jobs/:id/upload` — upload a Docker image tar for a job.
pub async fn upload_job_file(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let row = state
        .db
        .jobs
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("job not found"))?;
    if row.hirer_id != user.user_id {
        return Err(ApiError::forbidden("not your job"));
    }

    let field = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("multipart error: {e}")))?
        .ok_or_else(|| ApiError::bad_request("no file field"))?;

    let file_type = field.name().unwrap_or("code").to_string();
    let data: Bytes = field
        .bytes()
        .await
        .map_err(|e| ApiError::bad_request(format!("read field error: {e}")))?;

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let sha256 = hex::encode(hasher.finalize());

    let storage_dir = PathBuf::from(state.storage_path.as_ref()).join(id.to_string());
    tokio::fs::create_dir_all(&storage_dir)
        .await
        .map_err(|e| ApiError::internal(format!("mkdir error: {e}")))?;

    let file_path = storage_dir.join(format!("{file_type}.tar"));
    let file_size = data.len() as i64;
    tokio::fs::write(&file_path, &data)
        .await
        .map_err(|e| ApiError::internal(format!("write error: {e}")))?;

    let path_str = file_path
        .to_str()
        .ok_or_else(|| ApiError::internal("non-UTF8 path"))?
        .to_string();

    state
        .db
        .jobs
        .insert_file(id, &file_type, &path_str, file_size, &sha256)
        .await?;

    Ok(Json(serde_json::json!({
        "sha256":      sha256,
        "size_bytes":  file_size,
    })))
}

/// `POST /api/v1/jobs/:id/cancel` — cancel a pending job.
pub async fn cancel_job(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let row = state
        .db
        .jobs
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("job not found"))?;
    if row.hirer_id != user.user_id {
        return Err(ApiError::forbidden("not your job"));
    }
    if !matches!(row.status.as_str(), "pending" | "assigned") {
        return Err(ApiError::bad_request("cannot cancel job in current state"));
    }

    state
        .db
        .jobs
        .transition_status(id, &row.status, "cancelled", None, None, None)
        .await?;

    Ok(Json(serde_json::json!({ "cancelled": true })))
}

/// `GET /api/jobs/:id/result` — download the result file or logs for a completed job.
pub async fn download_result(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let row = state
        .db
        .jobs
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("job not found"))?;

    if row.hirer_id != user.user_id && user.role != "admin" {
        return Err(ApiError::forbidden("not your job"));
    }

    if row.status != "completed" {
        return Err(ApiError::bad_request("job is not completed yet"));
    }

    // Try to find result file
    let storage_dir = PathBuf::from(state.storage_path.as_ref()).join(id.to_string());
    let result_tar = storage_dir.join("result.tar.gz");
    let output_dir = storage_dir.join("output");

    if result_tar.exists() {
        let data = tokio::fs::read(&result_tar)
            .await
            .map_err(|e| ApiError::internal(format!("read result: {e}")))?;
        let filename = format!("job-{id}-result.tar.gz");
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            "application/gzip".parse().unwrap(),
        );
        headers.insert(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\"").parse().unwrap(),
        );
        return Ok((StatusCode::OK, headers, data).into_response());
    }

    if output_dir.exists() {
        // Return a listing of files or the first file found
        if let Ok(mut entries) = tokio::fs::read_dir(&output_dir).await {
            if let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let data = tokio::fs::read(&path)
                    .await
                    .map_err(|e| ApiError::internal(format!("read output: {e}")))?;
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("output")
                    .to_string();
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    "application/octet-stream".parse().unwrap(),
                );
                headers.insert(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{filename}\"").parse().unwrap(),
                );
                return Ok((StatusCode::OK, headers, data).into_response());
            }
        }
    }

    // Fall back: return logs as plain text
    let logs = state
        .db
        .jobs
        .get_logs_since(id, 0, 10_000)
        .await?;

    let mut text = String::new();
    for (ts, level, message) in &logs {
        text.push_str(&format!("[{ts}] [{level}] {message}\n"));
    }

    let filename = format!("job-{id}-output.txt");
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/plain; charset=utf-8".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"").parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, text.into_bytes()).into_response())
}

// ─── Conversions ──────────────────────────────────────────────────────────────

impl From<JobRow> for JobResponse {
    fn from(r: JobRow) -> Self {
        Self {
            id:                r.id,
            status:            r.status,
            gpu_backend:       r.gpu_backend,
            memory_limit_mb:   r.memory_limit_mb,
            max_duration_secs: r.max_duration_secs,
            cu_price:          r.cu_price,
            created_at:        r.created_at,
            worker_peer_id:    r.worker_peer_id,
        }
    }
}
