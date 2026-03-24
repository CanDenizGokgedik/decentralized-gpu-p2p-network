//! Linux network namespace management for job isolation.
//!
//! Only compiled on Linux. Uses the `nix` crate for namespace operations.

use anyhow::{Context, Result};
use tracing::{info, instrument};

/// Create a network namespace for a job.
///
/// The namespace is named `decentgpu-job-{job_id}`.
#[instrument(fields(job_id = %job_id))]
pub fn create_netns(job_id: &str) -> Result<String> {
    let ns_name = format!("decentgpu-job-{job_id}");

    // Create the namespace via `ip netns add`.
    // We use std::process::Command here rather than nix directly because
    // Linux netns manipulation requires specific ioctls that are simpler
    // to invoke through the `ip` utility.
    let status = std::process::Command::new("ip")
        .args(["netns", "add", &ns_name])
        .status()
        .context("running ip netns add")?;

    if !status.success() {
        anyhow::bail!("ip netns add failed with status {status}");
    }

    info!(ns_name = %ns_name, "network namespace created");
    Ok(ns_name)
}

/// Delete a network namespace previously created for a job.
#[instrument(fields(ns_name = %ns_name))]
pub fn delete_netns(ns_name: &str) -> Result<()> {
    let status = std::process::Command::new("ip")
        .args(["netns", "del", ns_name])
        .status()
        .context("running ip netns del")?;

    if !status.success() {
        anyhow::bail!("ip netns del failed with status {status}");
    }

    info!(ns_name = %ns_name, "network namespace deleted");
    Ok(())
}
