//! P2P networking layer for the master node.

pub mod behaviour;
pub mod handler;
pub mod protocols;

pub use handler::{JobCommand, WorkerEvent};

use anyhow::{Context, Result};
use dashmap::DashMap;
use decentgpu_common::config::MasterConfig;
use libp2p::{identity, noise, swarm::dial_opts::DialOpts, tcp, yamux, Multiaddr, Swarm};
use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::{broadcast, mpsc}, task::JoinHandle};
use uuid::Uuid;

use behaviour::MasterBehaviour;
use handler::MasterEventLoop;


async fn build_swarm(keypair: identity::Keypair) -> Result<Swarm<MasterBehaviour>> {
    let local_peer_id = libp2p::PeerId::from(keypair.public());
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )
        .context("TCP transport")?
        .with_websocket(noise::Config::new, yamux::Config::default)
        .await
        .context("WebSocket transport")?
        .with_relay_client(noise::Config::new, yamux::Config::default)
        .context("relay client")?
        .with_behaviour(|key, relay_client| MasterBehaviour::new(local_peer_id, key, relay_client))
        .map_err(|e| anyhow::anyhow!("behaviour: {e}"))?
        // Keep connections alive for 2 hours — prevents the master/worker connection from
        // dropping after a RegisterWorker or JobCompleted exchange (BUG 2).
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(7200)))
        .build();
    Ok(swarm)
}

/// Start the master P2P layer.
///
/// Returns:
/// - A join handle for the event loop task.
/// - A sender for `JobCommand`s (API layer → P2P).
/// - A receiver for `WorkerEvent`s (P2P → API / scheduler).
pub async fn start(
    cfg: MasterConfig,
) -> Result<(
    JoinHandle<Result<()>>,
    mpsc::Sender<JobCommand>,
    mpsc::Receiver<WorkerEvent>,
)> {
    start_with_auth(cfg, None, None, None).await
}

/// Start the master P2P layer with optional JWT auth and database for worker registration.
pub async fn start_with_auth(
    cfg:        MasterConfig,
    jwt_secret: Option<Vec<u8>>,
    db:         Option<crate::db::Database>,
    log_bus:    Option<Arc<DashMap<Uuid, broadcast::Sender<serde_json::Value>>>>,
) -> Result<(
    JoinHandle<Result<()>>,
    mpsc::Sender<JobCommand>,
    mpsc::Receiver<WorkerEvent>,
)> {
    // Read MASTER_KEYPAIR_PATH directly from env (single underscore) so the common
    // env var form works without the config-crate double-underscore separator.
    let keypair_path = decentgpu_common::keypair::keypair_path_from_env(
        "MASTER_KEYPAIR_PATH",
        "./master.keypair",
    );
    let keypair = decentgpu_common::keypair::load_or_generate(&keypair_path)?;
    tracing::info!(
        peer_id = %keypair.public().to_peer_id(),
        keypair_path = %keypair_path.display(),
        "master identity established"
    );
    let mut swarm = build_swarm(keypair).await?;

    // Listen on configured addresses.
    let tcp_addr: Multiaddr = cfg.p2p_tcp_addr.parse().context("p2p_tcp_addr")?;
    let quic_addr: Multiaddr = cfg.p2p_quic_addr.parse().context("p2p_quic_addr")?;

    // Extract the TCP port so we can register a loopback external address.
    // This ensures the rendezvous server accepts registration even before identify runs.
    let tcp_port = tcp_addr.iter().find_map(|proto| {
        if let libp2p::multiaddr::Protocol::Tcp(port) = proto { Some(port) } else { None }
    }).unwrap_or(9010);

    swarm.listen_on(tcp_addr).context("listen TCP")?;
    // QUIC is removed when WebSocket transport is in use (libp2p builder side effect).
    if let Err(e) = swarm.listen_on(quic_addr.clone()) {
        tracing::warn!(addr = %quic_addr, err = %e, "QUIC listener unavailable (non-fatal, using TCP+WS)");
    }

    let ws_port = std::env::var("MASTER_WS_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(9012);
    let ws_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{ws_port}/ws")
        .parse()
        .context("invalid master ws_addr")?;
    if let Err(e) = swarm.listen_on(ws_addr.clone()) {
        tracing::warn!(addr = %ws_addr, err = %e, "Master WebSocket listener failed (non-fatal)");
    } else {
        tracing::info!(addr = %ws_addr, "master listening on WebSocket");
    }

    let external_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{tcp_port}")
        .parse()
        .context("master external_addr")?;
    swarm.add_external_address(external_addr.clone());
    tracing::info!(addr = %external_addr, "added initial external address for rendezvous");

    // Dial bootstrap eagerly.
    // Use allocate_new_port() so libp2p does NOT try to reuse the master's own
    // listen port (9010) when connecting to the bootstrap (9000). On macOS,
    // port-reuse dialing to a local address causes "Address already in use (os error 48)".
    let bootstrap_addr: Multiaddr = cfg.bootstrap_addr.parse().context("bootstrap_addr")?;
    let dial_opts = DialOpts::unknown_peer_id()
        .address(bootstrap_addr.clone())
        .allocate_new_port()
        .build();
    swarm.dial(dial_opts).context("dial bootstrap")?;

    let (worker_tx, worker_rx) = mpsc::channel::<WorkerEvent>(256);
    let (job_tx, job_rx) = mpsc::channel::<JobCommand>(64);

    let mut event_loop = MasterEventLoop::new(swarm, bootstrap_addr, worker_tx, job_rx);
    if let (Some(secret), Some(database)) = (jwt_secret, db) {
        event_loop = event_loop.with_auth(secret, database);
    }
    if let Some(lb) = log_bus {
        event_loop = event_loop.with_log_bus(lb);
    }

    let handle = tokio::spawn(event_loop.run());

    Ok((handle, job_tx, worker_rx))
}
