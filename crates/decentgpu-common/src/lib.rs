#![deny(clippy::all)]

//! Shared types, configuration, and error definitions for DecentGPU.

pub mod config;
pub mod error;
pub mod keypair;
pub mod transfer;
pub mod types;

pub use error::DecentGpuError;
