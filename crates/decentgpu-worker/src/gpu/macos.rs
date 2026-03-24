//! macOS GPU detection via system_profiler.

use anyhow::Result;
use decentgpu_common::types::{GpuBackend, GpuInfo};
use tracing::warn;

/// Detect GPUs on macOS.
pub async fn detect_gpus() -> Result<Vec<GpuInfo>> {
    let mut gpus = Vec::new();

    // Determine backend: Apple Silicon uses Metal exclusively.
    let backend = if std::env::consts::ARCH == "aarch64" {
        GpuBackend::Metal
    } else {
        GpuBackend::Metal // Intel Macs with Metal-capable GPU
    };

    match detect_via_system_profiler().await {
        Ok(mut detected) => gpus.append(&mut detected),
        Err(e) => {
            warn!(error = %e, "system_profiler detection failed, using fallback");
            gpus.push(GpuInfo {
                name: "Apple GPU".into(),
                vram_mb: 0,
                backend,
            });
        }
    }

    if gpus.is_empty() {
        gpus.push(GpuInfo {
            name: "CPU".into(),
            vram_mb: 0,
            backend: GpuBackend::CpuOnly,
        });
    }

    Ok(gpus)
}

/// Parse `system_profiler SPDisplaysDataType -json` output.
async fn detect_via_system_profiler() -> Result<Vec<GpuInfo>> {
    let output = tokio::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("system_profiler failed with status {}", output.status);
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let displays = json
        .get("SPDisplaysDataType")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let backend = if std::env::consts::ARCH == "aarch64" {
        GpuBackend::Metal
    } else {
        GpuBackend::Metal
    };

    let gpus = displays
        .iter()
        .filter_map(|entry| {
            let name = entry
                .get("sppci_model")
                .and_then(|v| v.as_str())
                .unwrap_or("Apple GPU")
                .to_string();

            // VRAM: Apple Silicon unified memory is reported differently.
            let vram_mb = entry
                .get("spdisplays_vram")
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    let s = s.to_lowercase();
                    if let Some(n) = s.strip_suffix(" gb") {
                        n.trim().parse::<u64>().ok().map(|gb| gb * 1024)
                    } else if let Some(n) = s.strip_suffix(" mb") {
                        n.trim().parse::<u64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            Some(GpuInfo {
                name,
                vram_mb,
                backend,
            })
        })
        .collect();

    Ok(gpus)
}
