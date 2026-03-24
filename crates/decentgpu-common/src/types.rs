//! Shared domain types used across all DecentGPU crates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User role within the platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Hirer,
    Worker,
    Both,
}

impl UserRole {
    /// Return the database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Hirer => "hirer",
            UserRole::Worker => "worker",
            UserRole::Both => "both",
        }
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for UserRole {
    type Err = crate::error::DecentGpuError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hirer" => Ok(UserRole::Hirer),
            "worker" => Ok(UserRole::Worker),
            "both" => Ok(UserRole::Both),
            other => Err(crate::error::DecentGpuError::validation(format!(
                "unknown role: {other}"
            ))),
        }
    }
}

/// GPU compute backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuBackend {
    CpuOnly,
    Cuda,
    Metal,
    Rocm,
}

impl GpuBackend {
    /// Return the database/API string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            GpuBackend::CpuOnly => "cpu_only",
            GpuBackend::Cuda => "cuda",
            GpuBackend::Metal => "metal",
            GpuBackend::Rocm => "rocm",
        }
    }

    /// Return the GPU multiplier used for pricing.
    pub fn price_multiplier(self) -> f64 {
        match self {
            GpuBackend::CpuOnly => 1.0,
            GpuBackend::Cuda => 5.0,
            GpuBackend::Metal => 3.0,
            GpuBackend::Rocm => 4.0,
        }
    }
}

impl std::fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for GpuBackend {
    type Err = crate::error::DecentGpuError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cpu_only" => Ok(GpuBackend::CpuOnly),
            "cuda" => Ok(GpuBackend::Cuda),
            "metal" => Ok(GpuBackend::Metal),
            "rocm" => Ok(GpuBackend::Rocm),
            other => Err(crate::error::DecentGpuError::validation(format!(
                "unknown gpu backend: {other}"
            ))),
        }
    }
}

/// Current status of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    /// Return the database string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Assigned => "assigned",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for JobStatus {
    type Err = crate::error::DecentGpuError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(JobStatus::Pending),
            "assigned" => Ok(JobStatus::Assigned),
            "running" => Ok(JobStatus::Running),
            "completed" => Ok(JobStatus::Completed),
            "failed" => Ok(JobStatus::Failed),
            "cancelled" => Ok(JobStatus::Cancelled),
            other => Err(crate::error::DecentGpuError::validation(format!(
                "unknown job status: {other}"
            ))),
        }
    }
}

/// Summary of a worker node's capabilities, stored as JSONB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCapabilities {
    /// GPU devices detected on this worker.
    pub gpus: Vec<GpuInfo>,
    /// CPU information.
    pub cpu: CpuInfo,
    /// Total RAM in megabytes.
    pub ram_mb: u64,
    /// Available disk in megabytes.
    pub disk_mb: u64,
    /// Operating system identifier.
    pub os: String,
    /// Worker binary version.
    pub worker_version: String,
}

/// Information about a single GPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU model name.
    pub name: String,
    /// VRAM in megabytes.
    pub vram_mb: u64,
    /// Compute backend available.
    pub backend: GpuBackend,
}

/// Information about the CPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU model name.
    pub model: String,
    /// Physical core count.
    pub cores: u32,
    /// Logical thread count.
    pub threads: u32,
    /// Base frequency in MHz.
    pub freq_mhz: u64,
}

/// A job submission from a hirer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    /// Unique job identifier.
    pub job_id: Uuid,
    /// Hirer's user ID.
    pub hirer_id: Uuid,
    /// Maximum allowed wall-clock duration.
    pub max_duration_secs: u64,
    /// Memory limit in megabytes.
    pub memory_limit_mb: u64,
    /// CPU usage cap as a percentage (0–100).
    pub cpu_limit_percent: u64,
    /// Required GPU backend.
    pub required_backend: GpuBackend,
    /// SHA-256 hash of the Docker image tar.
    pub image_hash: String,
    /// Size of the Docker image tar in bytes.
    pub image_size_bytes: u64,
}

/// Credit amount type (in platform credit units).
pub type Credits = i64;

/// Credit transaction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxType {
    Deposit,
    JobCharge,
    JobEarn,
    Refund,
}

impl TxType {
    /// Return the database string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            TxType::Deposit => "deposit",
            TxType::JobCharge => "job_charge",
            TxType::JobEarn => "job_earn",
            TxType::Refund => "refund",
        }
    }
}

impl std::fmt::Display for TxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A log line emitted during job execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLogEntry {
    /// Associated job ID.
    pub job_id: Uuid,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: i64,
    /// Severity level.
    pub level: String,
    /// Log message body.
    pub message: String,
}

/// Worker info returned from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    /// libp2p PeerId as a string.
    pub peer_id: String,
    /// Owning user ID.
    pub user_id: Uuid,
    /// Worker capabilities.
    pub capabilities: WorkerCapabilities,
    /// Uptime score (0.0–100.0).
    pub uptime_score: f64,
    /// Total jobs completed.
    pub jobs_completed: i64,
    /// Whether the worker is currently online.
    pub is_online: bool,
    /// Last time the master received a heartbeat.
    pub last_seen: Option<DateTime<Utc>>,
}

/// Pricing configuration constants.
pub mod pricing {
    /// Base rate in credits per hour for CPU-only jobs.
    pub const BASE_RATE_PER_HOUR: i64 = 10;

    /// Platform fee percentage retained on job completion (15%).
    pub const PLATFORM_FEE_BPS: i64 = 1500; // basis points

    /// Worker payout fraction after platform fee (85%).
    pub const WORKER_PAYOUT_FRACTION: f64 = 0.85;
}
