//! Linux GPU detection via nvidia-smi and /proc filesystem.

use anyhow::Result;
use decentgpu_common::types::{GpuBackend, GpuInfo};
use tracing::{debug, warn};

/// Detect GPUs on Linux.
pub async fn detect_gpus() -> Result<Vec<GpuInfo>> {
    let mut gpus = Vec::new();

    // NVIDIA detection.
    if std::path::Path::new("/proc/driver/nvidia").exists() {
        match detect_nvidia_gpus().await {
            Ok(mut nvidia_gpus) => gpus.append(&mut nvidia_gpus),
            Err(e) => warn!(error = %e, "nvidia-smi detection failed"),
        }
    }

    // AMD ROCm detection.
    if std::path::Path::new("/dev/kfd").exists() {
        gpus.push(GpuInfo {
            name: "AMD GPU (ROCm)".into(),
            vram_mb: 0,
            backend: GpuBackend::Rocm,
        });
        debug!("AMD ROCm GPU detected via /dev/kfd");
    }

    // Fallback to CPU only if no GPU found.
    if gpus.is_empty() {
        gpus.push(GpuInfo {
            name: "CPU".into(),
            vram_mb: 0,
            backend: GpuBackend::CpuOnly,
        });
    }

    Ok(gpus)
}

/// Query nvidia-smi for GPU names and VRAM.
async fn detect_nvidia_gpus() -> Result<Vec<GpuInfo>> {
    let output = tokio::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("nvidia-smi exited with status {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let gpus = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, ", ").collect();
            if parts.len() != 2 {
                return None;
            }
            let name = parts[0].trim().to_string();
            let vram_mb: u64 = parts[1].trim().parse().unwrap_or(0);
            Some(GpuInfo {
                name,
                vram_mb,
                backend: GpuBackend::Cuda,
            })
        })
        .collect();

    Ok(gpus)
}
