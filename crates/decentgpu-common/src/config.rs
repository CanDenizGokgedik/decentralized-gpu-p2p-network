//! Shared configuration structs loaded from environment variables.

use serde::{Deserialize, Serialize};

/// Configuration for the Bootstrap node.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BootstrapConfig {
    /// TCP listen address, e.g. `/ip4/0.0.0.0/tcp/9000`
    #[serde(default = "BootstrapConfig::default_tcp_addr")]
    pub tcp_addr: String,

    /// QUIC listen address, e.g. `/ip4/0.0.0.0/udp/9000/quic-v1`
    #[serde(default = "BootstrapConfig::default_quic_addr")]
    pub quic_addr: String,

    /// HTTP health check port.
    #[serde(default = "BootstrapConfig::default_health_port")]
    pub health_port: u16,

    /// Path to persist keypair (optional, generates ephemeral if absent).
    pub keypair_path: Option<String>,
}

impl BootstrapConfig {
    fn default_tcp_addr() -> String {
        "/ip4/0.0.0.0/tcp/9000".into()
    }
    fn default_quic_addr() -> String {
        "/ip4/0.0.0.0/udp/9000/quic-v1".into()
    }
    fn default_health_port() -> u16 {
        9001
    }
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            tcp_addr: Self::default_tcp_addr(),
            quic_addr: Self::default_quic_addr(),
            health_port: Self::default_health_port(),
            keypair_path: None,
        }
    }
}

/// Configuration for the Master node.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MasterConfig {
    /// Bootstrap node multiaddr.
    #[serde(default = "MasterConfig::default_bootstrap_addr")]
    pub bootstrap_addr: String,

    /// TCP listen address for P2P.
    #[serde(default = "MasterConfig::default_p2p_tcp_addr")]
    pub p2p_tcp_addr: String,

    /// QUIC listen address for P2P.
    #[serde(default = "MasterConfig::default_p2p_quic_addr")]
    pub p2p_quic_addr: String,

    /// HTTP API listen address.
    #[serde(default = "MasterConfig::default_api_addr")]
    pub api_addr: String,

    /// PostgreSQL connection URL. Empty string disables the database / API.
    #[serde(default)]
    pub database_url: String,

    /// JWT secret key (min 32 bytes). Defaults to an insecure placeholder.
    #[serde(default = "MasterConfig::default_jwt_secret")]
    pub jwt_secret: String,

    /// JWT expiry in seconds.
    #[serde(default = "MasterConfig::default_jwt_expiry_secs")]
    pub jwt_expiry_secs: u64,

    /// Path to persist keypair.
    pub keypair_path: Option<String>,

    /// Filesystem path for storing uploaded job files.
    #[serde(default = "MasterConfig::default_storage_path")]
    pub storage_path: String,

    /// Directory containing pre-built worker binaries for download.
    /// Defaults to `{storage_path}/binaries`.
    pub binaries_dir: Option<String>,

    /// Public IP address of this master node, used when generating worker
    /// setup scripts so workers can dial the correct address.
    /// Env var: `MASTER__PUBLIC_IP` (config crate) or `MASTER_PUBLIC_IP`.
    /// Defaults to "127.0.0.1".
    #[serde(default = "MasterConfig::default_public_ip")]
    pub public_ip: String,
}

impl MasterConfig {
    fn default_jwt_secret() -> String {
        "CHANGE_ME_BEFORE_PRODUCTION_USE_32_BYTES".into()
    }
    fn default_bootstrap_addr() -> String {
        "/ip4/127.0.0.1/tcp/9000".into()
    }
    fn default_p2p_tcp_addr() -> String {
        "/ip4/0.0.0.0/tcp/9010".into()
    }
    fn default_p2p_quic_addr() -> String {
        "/ip4/0.0.0.0/udp/9010/quic-v1".into()
    }
    fn default_api_addr() -> String {
        "0.0.0.0:8888".into()
    }
    fn default_jwt_expiry_secs() -> u64 {
        86400
    }
    fn default_storage_path() -> String {
        "/var/decentgpu/storage".into()
    }
    fn default_public_ip() -> String {
        "127.0.0.1".into()
    }
}

/// Configuration for a Worker node.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkerConfig {
    /// Bootstrap node multiaddr.
    #[serde(default = "WorkerConfig::default_bootstrap_addr")]
    pub bootstrap_addr: String,

    /// TCP listen address for P2P.
    #[serde(default = "WorkerConfig::default_p2p_tcp_addr")]
    pub p2p_tcp_addr: String,

    /// QUIC listen address for P2P.
    #[serde(default = "WorkerConfig::default_p2p_quic_addr")]
    pub p2p_quic_addr: String,

    /// Path to persist keypair.
    pub keypair_path: Option<String>,

    /// Docker host socket path (Linux/macOS) or named pipe (Windows).
    #[serde(default = "WorkerConfig::default_docker_socket")]
    pub docker_socket: String,

    /// Temporary directory for job files.
    #[serde(default = "WorkerConfig::default_tmp_dir")]
    pub tmp_dir: String,

    /// Optional user_id to associate this worker with in the master DB.
    pub user_id: Option<String>,

    /// Heartbeat interval in seconds.
    #[serde(default = "WorkerConfig::default_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,

    /// Optional direct multiaddr for the master node (e.g. `/ip4/1.2.3.4/tcp/9010`).
    /// When set, the worker dials the master directly after rendezvous registration so
    /// the master can discover it without waiting for a rendezvous discover cycle.
    /// Env var: `WORKER_MASTER_ADDR`
    pub master_addr: Option<String>,
}

impl WorkerConfig {
    fn default_bootstrap_addr() -> String {
        "/ip4/127.0.0.1/tcp/9000".into()
    }
    fn default_p2p_tcp_addr() -> String {
        "/ip4/0.0.0.0/tcp/9020".into()
    }
    fn default_p2p_quic_addr() -> String {
        "/ip4/0.0.0.0/udp/9020/quic-v1".into()
    }
    fn default_docker_socket() -> String {
        #[cfg(windows)]
        return "\\\\.\\pipe\\docker_engine".into();
        #[cfg(not(windows))]
        "/var/run/docker.sock".into()
    }
    fn default_tmp_dir() -> String {
        "/tmp/decentgpu".into()
    }
    fn default_heartbeat_interval_secs() -> u64 {
        30
    }
}

/// Load a config struct from environment variables and an optional config file.
///
/// Environment variables override file values. Variable names are uppercased
/// and use `__` as a separator, e.g. `MASTER__DATABASE_URL`.
pub fn load_config<'de, T>(prefix: &str) -> Result<T, config::ConfigError>
where
    T: serde::de::DeserializeOwned,
{
    config::Config::builder()
        .add_source(
            config::Environment::with_prefix(prefix)
                .separator("__")
                .try_parsing(true),
        )
        .build()?
        .try_deserialize()
}
