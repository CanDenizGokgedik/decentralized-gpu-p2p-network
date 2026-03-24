//! P2P networking layer for the worker node.

pub mod behaviour;
pub mod handler;

use anyhow::{Context, Result};
use decentgpu_common::{config::WorkerConfig, types::WorkerCapabilities};
use handler::{JobResult, WorkerEventLoop};
use libp2p::{identity, noise, tcp, yamux, Multiaddr, Swarm};
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};

use behaviour::WorkerBehaviour;


async fn build_swarm(keypair: identity::Keypair) -> Result<Swarm<WorkerBehaviour>> {
    let local_peer_id = libp2p::PeerId::from(keypair.public());

    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_dns()?
        .with_websocket(noise::Config::new, yamux::Config::default)
        .await?
        .with_relay_client(noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key, relay_client| {
            WorkerBehaviour::new(local_peer_id, key, relay_client)
        })?
        .with_swarm_config(|config: libp2p::swarm::Config| {
            config.with_idle_connection_timeout(Duration::from_secs(7200))
        })
        .build();

    Ok(swarm)
}

/// Try to find a free TCP port starting from `start`, scanning up to 20 ports.
async fn find_free_port(start: u16) -> u16 {
    for port in start..start.saturating_add(20) {
        if tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .is_ok()
        {
            return port;
        }
        tracing::debug!(port, "port busy, trying next");
    }
    start.saturating_add(20)
}

/// Extract the port number from a multiaddr string like `/ip4/0.0.0.0/tcp/9020`
/// or `/ip4/0.0.0.0/udp/9020/quic-v1`. Returns None if not parseable.
fn extract_port_from_multiaddr(addr: &str) -> Option<u16> {
    // Split on '/' and look for a numeric component after tcp or udp
    let parts: Vec<&str> = addr.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "tcp" || *part == "udp" {
            if let Some(port_str) = parts.get(i + 1) {
                if let Ok(port) = port_str.parse::<u16>() {
                    return Some(port);
                }
            }
        }
    }
    None
}

/// Start the worker P2P layer.
///
/// Returns:
/// - A join handle for the event loop task.
/// - A sender for incoming `JobAssignment`s (P2P → executor).
/// - A receiver for `JobResult`s (executor → P2P).
pub async fn start(
    cfg: WorkerConfig,
    capabilities: WorkerCapabilities,
) -> Result<(
    JoinHandle<Result<()>>,
    mpsc::Receiver<decentgpu_proto::JobAssignment>,
    mpsc::Sender<JobResult>,
)> {
    // Read WORKER_AUTH_TOKEN from environment.
    let auth_token = std::env::var("WORKER_AUTH_TOKEN").ok();
    if auth_token.is_none() {
        tracing::warn!(
            "WORKER_AUTH_TOKEN not set — worker will not be linked to a user account"
        );
    }

    // WORKER_RESET_IDENTITY=true — delete stale keypair so a fresh peer_id is used.
    // Clears "Unexpected peer ID" DHT errors after restarts with stale bootstrap cache.
    let keypair_path = decentgpu_common::keypair::keypair_path_from_env(
        "WORKER_KEYPAIR_PATH",
        "./worker.keypair",
    );
    if std::env::var("WORKER_RESET_IDENTITY").as_deref() == Ok("true") {
        if keypair_path.exists() {
            std::fs::remove_file(&keypair_path).ok();
            tracing::info!(path = %keypair_path.display(), "WORKER_RESET_IDENTITY=true — deleted stale keypair");
        }
    }
    let keypair = decentgpu_common::keypair::load_or_generate(&keypair_path)?;
    tracing::info!(
        peer_id = %keypair.public().to_peer_id(),
        keypair_path = %keypair_path.display(),
        "worker identity established"
    );
    let mut swarm = build_swarm(keypair).await?;

    // Auto port selection: extract preferred ports from config and find free ones.
    let preferred_tcp_port = extract_port_from_multiaddr(&cfg.p2p_tcp_addr).unwrap_or(9020);
    let preferred_quic_port = extract_port_from_multiaddr(&cfg.p2p_quic_addr).unwrap_or(9020);

    let tcp_port = find_free_port(preferred_tcp_port).await;
    if tcp_port != preferred_tcp_port {
        tracing::warn!(
            preferred = preferred_tcp_port,
            using = tcp_port,
            "preferred TCP port busy, using alternative"
        );
    }

    let quic_port = find_free_port(preferred_quic_port.max(tcp_port + 1)).await;
    if quic_port != preferred_quic_port {
        tracing::warn!(
            preferred = preferred_quic_port,
            using = quic_port,
            "preferred QUIC port busy, using alternative"
        );
    }

    let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{tcp_port}")
        .parse()
        .context("p2p_tcp_addr")?;
    let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{quic_port}/quic-v1")
        .parse()
        .context("p2p_quic_addr")?;

    swarm.listen_on(tcp_addr).context("listen TCP")?;
    // QUIC is removed when WebSocket transport is in use (libp2p builder side effect).
    if let Err(e) = swarm.listen_on(quic_addr.clone()) {
        tracing::warn!(addr = %quic_addr, err = %e, "QUIC listener unavailable (non-fatal, using TCP+WS)");
    }

    // Register a loopback external address so the rendezvous server accepts our registration.
    // The identify protocol will later provide the observed external address via the bootstrap
    // peer; `on_bootstrap_identified` will add those too before calling `do_register`.
    let external_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{tcp_port}")
        .parse()
        .context("external_addr")?;
    swarm.add_external_address(external_addr.clone());
    tracing::info!(addr = %external_addr, "added initial external address for rendezvous");

    let bootstrap_addr: Multiaddr = cfg.bootstrap_addr.parse().context("bootstrap_addr")?;
    swarm.dial(bootstrap_addr.clone()).context("dial bootstrap")?;

    let (job_tx, job_rx) = mpsc::channel::<decentgpu_proto::JobAssignment>(32);
    let (result_tx, result_rx) = mpsc::channel::<JobResult>(32);

    // Prefer WORKER_MASTER_ADDR env var (single-underscore form, consistent with
    // WORKER_AUTH_TOKEN) over the config-crate field which requires WORKER__MASTER_ADDR.
    let master_addr = std::env::var("WORKER_MASTER_ADDR")
        .ok()
        .or_else(|| cfg.master_addr.clone());
    if let Some(ref addr) = master_addr {
        tracing::info!(master_addr = %addr, "master_addr configured — will dial after rendezvous registration");
    } else {
        tracing::warn!("WORKER_MASTER_ADDR not set — worker will not dial master directly");
    }
    let event_loop = WorkerEventLoop::new(swarm, bootstrap_addr, capabilities, job_tx, result_rx, auth_token, master_addr);

    let handle = tokio::spawn(event_loop.run());

    Ok((handle, job_rx, result_tx))
}
