//! Build Docker images from uploaded code files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bollard::{image::BuildImageOptions, Docker};
use decentgpu_common::types::GpuBackend;
use futures::StreamExt as _;
use sha2::{Digest, Sha256};
use tracing::{debug, info};

/// Builds Docker images from user-uploaded source files.
#[derive(Clone)]
pub struct DockerBuilder {
    /// Root directory for build contexts (e.g. /var/decentgpu/storage).
    work_dir: PathBuf,
    client:   std::sync::Arc<Docker>,
}

impl DockerBuilder {
    /// Connect to the local Docker daemon.
    pub fn new(work_dir: impl Into<PathBuf>) -> Result<Self> {
        let client = Docker::connect_with_local_defaults()
            .context("connect to Docker daemon")?;
        Ok(Self {
            work_dir: work_dir.into(),
            client:   std::sync::Arc::new(client),
        })
    }

    /// Build a Docker image from uploaded `code.py` + `requirements.txt`.
    ///
    /// Returns the path to the exported `.tar` image file.
    pub async fn build_image(
        &self,
        job_id:            &str,
        code_path:         &Path,
        requirements_path: &Path,
        gpu_backend:       GpuBackend,
    ) -> Result<PathBuf> {
        // Use a dedicated `build/` subdirectory so we never overwrite the original
        // source files that live alongside the job directory (same work_dir/job_id path).
        let build_dir = self.work_dir.join(job_id).join("build");
        tokio::fs::create_dir_all(&build_dir).await?;

        // Copy user files into build context.
        tokio::fs::copy(code_path, build_dir.join("code.py")).await?;
        tokio::fs::copy(requirements_path, build_dir.join("requirements.txt")).await?;

        // Generate Dockerfile.
        let base_image = match gpu_backend {
            GpuBackend::Cuda    => "nvidia/cuda:12.3.0-runtime-ubuntu22.04",
            GpuBackend::Rocm    => "rocm/pytorch:latest",
            GpuBackend::Metal | GpuBackend::CpuOnly => "python:3.11-slim",
        };
        let dockerfile = format!(
            "FROM {base_image}\n\
             WORKDIR /workspace\n\
             RUN mkdir -p /workspace/output\n\
             COPY requirements.txt .\n\
             RUN pip install --no-cache-dir -r requirements.txt\n\
             COPY code.py .\n\
             CMD [\"python\", \"-u\", \"code.py\"]\n"
        );
        tokio::fs::write(build_dir.join("Dockerfile"), &dockerfile).await?;

        // Create a tar of the build context in memory.
        let context_bytes = tar_directory(&build_dir).await?;

        let tag = format!("decentgpu-job-{job_id}:latest");
        let options = BuildImageOptions {
            dockerfile: "Dockerfile",
            t:          &tag,
            rm:         true,
            ..Default::default()
        };

        info!(%job_id, %tag, "building docker image");
        let mut build_stream = self.client.build_image(
            options,
            None,
            Some(bytes::Bytes::from(context_bytes).into()),
        );

        while let Some(item) = build_stream.next().await {
            match item {
                Ok(info) => {
                    if let Some(s) = info.stream {
                        debug!(output = s.trim_end(), "docker build");
                    }
                    if let Some(err) = info.error {
                        anyhow::bail!("docker build error: {err}");
                    }
                }
                Err(e) => anyhow::bail!("build stream error: {e}"),
            }
        }

        // Export image to tar.
        let image_tar = self.work_dir.join(job_id).join("image.tar");
        let mut export_stream = self.client.export_image(&tag);
        let mut data = Vec::new();
        while let Some(chunk) = export_stream.next().await {
            data.extend_from_slice(
                &chunk.map_err(|e| anyhow::anyhow!("export error: {e}"))?,
            );
        }
        tokio::fs::write(&image_tar, &data).await?;

        let size = data.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hex::encode(hasher.finalize());

        info!(%job_id, path = %image_tar.display(), size, %hash, "image built and exported");
        Ok(image_tar)
    }
}

/// Create an in-memory tar of a directory (files only, no subdirs).
async fn tar_directory(dir: &Path) -> Result<Vec<u8>> {
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        if entry.file_type().await?.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            let data = tokio::fs::read(entry.path()).await?;
            entries.push((name, data));
        }
    }

    // Build tar in memory.
    let mut ar = tar::Builder::new(Vec::new());
    for (name, data) in &entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        ar.append_data(&mut header, name, data.as_slice())?;
    }
    Ok(ar.into_inner()?)
}
