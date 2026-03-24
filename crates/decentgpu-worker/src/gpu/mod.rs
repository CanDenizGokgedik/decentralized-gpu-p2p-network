//! GPU detection — dispatches to the platform-specific implementation.

pub mod detector;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

use anyhow::Result;
use decentgpu_common::types::WorkerCapabilities;

/// Detect all GPU and CPU capabilities on the current machine.
///
/// Returns a fully populated [`WorkerCapabilities`] struct.
pub async fn detect() -> Result<WorkerCapabilities> {
    detector::detect_capabilities().await
}
