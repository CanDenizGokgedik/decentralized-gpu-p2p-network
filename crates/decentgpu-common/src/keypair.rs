//! Shared keypair utilities — load-or-generate a persistent Ed25519 identity.
//!
//! Both bootstrap and master use this module so that:
//! 1. The peer ID is stable across restarts (keypair persisted to disk).
//! 2. The env-var name uses a SINGLE underscore (`BOOTSTRAP_KEYPAIR_PATH`,
//!    `MASTER_KEYPAIR_PATH`) — readable without the double-underscore separator
//!    that the `config` crate requires.

use std::path::{Path, PathBuf};
use libp2p::identity::Keypair;

/// Load a keypair from `path` if it exists, or generate a new Ed25519 keypair,
/// persist it to `path`, and return it.
///
/// Parent directories are created automatically.
pub fn load_or_generate(path: &Path) -> anyhow::Result<Keypair> {
    if path.exists() {
        let bytes = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("read keypair {}: {e}", path.display()))?;
        let kp = Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| anyhow::anyhow!("decode keypair {}: {e}", path.display()))?;
        tracing::info!(
            path = %path.display(),
            peer_id = %kp.public().to_peer_id(),
            "loaded existing keypair — peer ID is stable"
        );
        Ok(kp)
    } else {
        // Create parent directory if needed.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("create keypair dir {}: {e}", parent.display()))?;
            }
        }
        let kp = Keypair::generate_ed25519();
        let bytes = kp.to_protobuf_encoding()
            .map_err(|e| anyhow::anyhow!("encode keypair: {e}"))?;
        std::fs::write(path, &bytes)
            .map_err(|e| anyhow::anyhow!("write keypair {}: {e}", path.display()))?;
        tracing::info!(
            path = %path.display(),
            peer_id = %kp.public().to_peer_id(),
            "generated and persisted new keypair — peer ID is now permanent"
        );
        Ok(kp)
    }
}

/// Resolve the keypair file path from an environment variable (single-underscore
/// form such as `BOOTSTRAP_KEYPAIR_PATH`) with a fallback default path.
///
/// Using `std::env::var` directly bypasses the `config` crate's double-underscore
/// separator so both `BOOTSTRAP_KEYPAIR_PATH=./bootstrap.keypair` and
/// `BOOTSTRAP__KEYPAIR_PATH=./bootstrap.keypair` work.
pub fn keypair_path_from_env(env_var: &str, default: &str) -> PathBuf {
    match std::env::var(env_var) {
        Ok(val) if !val.is_empty() => {
            tracing::debug!(env_var, path = %val, "keypair path from env");
            PathBuf::from(val)
        }
        _ => {
            tracing::warn!(
                env_var,
                default,
                "env var not set — using default keypair path"
            );
            PathBuf::from(default)
        }
    }
}
