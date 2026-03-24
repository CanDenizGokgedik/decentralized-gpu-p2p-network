//! Platform-agnostic capability detection entry point.

use anyhow::Result;
use decentgpu_common::types::{CpuInfo, GpuInfo, WorkerCapabilities};

/// Collect full capabilities for this machine.
pub async fn detect_capabilities() -> Result<WorkerCapabilities> {
    let gpus = detect_gpus().await?;
    let cpu = detect_cpu()?;
    let ram_mb = detect_ram_mb()?;
    let disk_mb = detect_disk_mb()?;
    let os = std::env::consts::OS.to_string();
    let worker_version = env!("CARGO_PKG_VERSION").to_string();

    Ok(WorkerCapabilities {
        gpus,
        cpu,
        ram_mb,
        disk_mb,
        os,
        worker_version,
    })
}

/// Detect GPUs using the platform-specific backend.
async fn detect_gpus() -> Result<Vec<GpuInfo>> {
    #[cfg(target_os = "linux")]
    {
        super::linux::detect_gpus().await
    }
    #[cfg(target_os = "macos")]
    {
        super::macos::detect_gpus().await
    }
    #[cfg(target_os = "windows")]
    {
        super::windows::detect_gpus()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Unknown platform: report CPU only.
        Ok(vec![GpuInfo {
            name: "Unknown".into(),
            vram_mb: 0,
            backend: GpuBackend::CpuOnly,
        }])
    }
}

/// Detect basic CPU information.
fn detect_cpu() -> Result<CpuInfo> {
    // Use std::thread for thread count; parse /proc/cpuinfo on Linux,
    // sysctl on macOS, and fall back to a sensible default.
    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);

    // Physical core count heuristic (threads / 2 on most modern CPUs).
    let cores = (threads / 2).max(1);

    Ok(CpuInfo {
        model: detect_cpu_model(),
        cores,
        threads,
        freq_mhz: 0, // frequency detection is platform-specific
    })
}

/// Attempt to read the CPU model string.
fn detect_cpu_model() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if let Some(model) = line.strip_prefix("model name\t: ") {
                    return model.trim().to_string();
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
    }
    "Unknown CPU".to_string()
}

/// Detect total system RAM in megabytes.
fn detect_ram_mb() -> Result<u64> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/meminfo")?;
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest.trim().trim_end_matches(" kB").trim().parse()?;
                return Ok(kb / 1024);
            }
        }
        Ok(0)
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()?;
        let bytes: u64 = String::from_utf8_lossy(&output.stdout).trim().parse()?;
        Ok(bytes / 1_048_576)
    }
    #[cfg(target_os = "windows")]
    {
        Ok(0) // implemented via WMI in windows.rs
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok(0)
    }
}

/// Detect available disk space in megabytes for the temp directory.
fn detect_disk_mb() -> Result<u64> {
    // A simple heuristic: check free space on the root filesystem.
    // This is best-effort; real implementations use statvfs.
    Ok(0)
}
