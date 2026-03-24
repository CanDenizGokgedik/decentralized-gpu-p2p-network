//! Post-job resource cleanup.

use anyhow::Result;
use std::path::Path;
use tracing::{info, warn};

use super::manager::DockerManager;

/// Remove container, image, and temporary job files.
///
/// This must be called regardless of job success or failure.
pub async fn cleanup_job(
    docker: &DockerManager,
    job_id: &str,
    container_id: Option<&str>,
    image_id: Option<&str>,
    tmp_dir: &Path,
) -> Result<()> {
    // Stop and remove container.
    if let Some(cid) = container_id {
        if let Err(e) = docker.remove_container(cid).await {
            warn!(container_id = %cid, error = %e, "container cleanup failed");
        } else {
            info!(container_id = %cid, "container cleaned up");
        }
    }

    // Remove image.
    if let Some(iid) = image_id {
        if let Err(e) = docker.remove_image(iid).await {
            warn!(image_id = %iid, error = %e, "image cleanup failed");
        } else {
            info!(image_id = %iid, "image cleaned up");
        }
    }

    // Remove temp directory.
    let job_tmp = tmp_dir.join(format!("job-{job_id}"));
    if job_tmp.exists() {
        if let Err(e) = tokio::fs::remove_dir_all(&job_tmp).await {
            warn!(path = %job_tmp.display(), error = %e, "tmp dir cleanup failed");
        } else {
            info!(path = %job_tmp.display(), "tmp dir cleaned up");
        }
    }

    Ok(())
}
