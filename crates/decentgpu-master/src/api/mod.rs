//! HTTP/WebSocket API for the master node.

pub mod admin;
pub mod auth;
pub mod credits;
pub mod downloads;
pub mod jobs;
pub mod logs;
pub mod router;
pub mod terminal;
pub mod workers;

use anyhow::Result;
use dashmap::DashMap;
use decentgpu_common::config::MasterConfig;
use socketioxide::SocketIo;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::info;
use uuid::Uuid;

use crate::{
    credits::ComputeUnitLedger,
    db::Database,
    docker::builder::DockerBuilder,
    p2p::JobCommand,
    scheduler::SchedulerHandle,
};

/// Shared application state injected into Axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// Repository aggregation (users, jobs, workers, compute_units).
    pub db: Database,
    /// Compute Unit ledger for atomic balance mutations.
    pub ledger: ComputeUnitLedger,
    /// Job scheduler handle — enqueue jobs for worker assignment.
    pub scheduler: SchedulerHandle,
    /// JWT secret bytes.
    pub jwt_secret: Arc<Vec<u8>>,
    /// JWT expiry in seconds.
    pub jwt_expiry_secs: u64,
    /// File storage root path.
    pub storage_path: Arc<String>,
    /// Socket.io layer for terminal streaming.
    pub socket_io: SocketIo,
    /// Live log broadcast bus: job_id → sender.
    pub log_bus: Arc<DashMap<Uuid, broadcast::Sender<serde_json::Value>>>,
    /// P2P command sender (for cancel / disconnect from the API layer).
    pub p2p_cmd_tx: mpsc::Sender<JobCommand>,
    /// Docker image builder (None when Docker daemon is unavailable).
    pub docker_builder: Option<Arc<DockerBuilder>>,
    /// Directory that contains pre-built worker binaries for download.
    pub binaries_dir: Arc<String>,
    /// Public IP of this master node, used in worker setup scripts.
    pub public_ip: Arc<String>,
    /// Bootstrap node address, used as default for worker setup scripts.
    pub bootstrap_addr: Arc<String>,
    /// P2P TCP listen address (e.g. `/ip4/0.0.0.0/tcp/9010`), used to extract
    /// the port when generating the public-facing master P2P address for workers.
    pub p2p_tcp_addr: Arc<String>,
}

impl AppState {
    /// Construct shared state from config, pool, scheduler handle, P2P sender, and log bus.
    pub async fn new(
        cfg:        MasterConfig,
        pool:       PgPool,
        scheduler:  SchedulerHandle,
        p2p_cmd_tx: mpsc::Sender<JobCommand>,
        log_bus:    Arc<DashMap<Uuid, broadcast::Sender<serde_json::Value>>>,
    ) -> Result<Self> {
        let (socket_io_layer, socket_io) = SocketIo::new_layer();
        let _socket_io_layer = socket_io_layer;

        let db     = Database::new(pool.clone());
        let ledger = ComputeUnitLedger::new(pool);

        let docker_builder = DockerBuilder::new(&cfg.storage_path).ok().map(Arc::new);
        if docker_builder.is_none() {
            tracing::warn!("Docker unavailable — image builds will be disabled");
        }

        let binaries_dir = cfg.binaries_dir.clone()
            .unwrap_or_else(|| format!("{}/binaries", cfg.storage_path));

        Ok(Self {
            db,
            ledger,
            scheduler,
            jwt_secret:     Arc::new(cfg.jwt_secret.into_bytes()),
            jwt_expiry_secs: cfg.jwt_expiry_secs,
            storage_path:   Arc::new(cfg.storage_path),
            socket_io,
            log_bus,
            p2p_cmd_tx,
            docker_builder,
            binaries_dir:   Arc::new(binaries_dir),
            public_ip:      Arc::new(cfg.public_ip),
            bootstrap_addr: Arc::new(cfg.bootstrap_addr),
            p2p_tcp_addr:   Arc::new(cfg.p2p_tcp_addr),
        })
    }
}

/// Start the HTTP API server.
pub async fn serve(cfg: MasterConfig, state: AppState) -> Result<()> {
    let app  = router::build(state);
    let addr = cfg.api_addr.clone();
    info!(addr = %addr, "API server starting");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
