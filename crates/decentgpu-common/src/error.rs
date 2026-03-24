//! Shared error types for the DecentGPU platform.

use thiserror::Error;

/// Top-level error type shared across all crates.
#[derive(Debug, Error)]
pub enum DecentGpuError {
    /// Database operation failed.
    #[error("database error: {0}")]
    Database(String),

    /// Resource not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Conflicting state (e.g. duplicate email, wrong status transition).
    #[error("conflict: {0}")]
    Conflict(String),

    /// Insufficient Compute Units to complete the operation.
    ///
    /// 1 CU = 1 hour of baseline CPU compute.
    #[error("insufficient compute units: available={available}, required={required}")]
    InsufficientComputeUnits { available: i64, required: i64 },

    /// Authentication or authorization failure.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Request validation failed.
    #[error("validation error: {0}")]
    Validation(String),

    /// A P2P networking error occurred.
    #[error("p2p error: {0}")]
    P2p(String),

    /// Docker/container operation failed.
    #[error("docker error: {0}")]
    Docker(String),

    /// Operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),

    /// Generic internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl DecentGpuError {
    pub fn database(msg: impl std::fmt::Display) -> Self { Self::Database(msg.to_string()) }
    pub fn not_found(msg: impl std::fmt::Display) -> Self { Self::NotFound(msg.to_string()) }
    pub fn conflict(msg: impl std::fmt::Display) -> Self { Self::Conflict(msg.to_string()) }
    pub fn unauthorized(msg: impl std::fmt::Display) -> Self { Self::Unauthorized(msg.to_string()) }
    pub fn validation(msg: impl std::fmt::Display) -> Self { Self::Validation(msg.to_string()) }
    pub fn p2p(msg: impl std::fmt::Display) -> Self { Self::P2p(msg.to_string()) }
    pub fn docker(msg: impl std::fmt::Display) -> Self { Self::Docker(msg.to_string()) }
    pub fn internal(msg: impl std::fmt::Display) -> Self { Self::Internal(msg.to_string()) }
    pub fn timeout(msg: impl std::fmt::Display) -> Self { Self::Timeout(msg.to_string()) }
}
