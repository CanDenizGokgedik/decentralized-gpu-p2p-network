#![deny(clippy::all)]

//! DecentGPU Bootstrap Node
//!
//! Provides Rendezvous + Relay v2 services so that workers and the master
//! node can discover each other regardless of NAT.

mod behaviour;

use anyhow::{Context, Result};
use axum::{extract::State, routing::get, Json, Router};
use decentgpu_common::config::{load_config, BootstrapConfig};
use futures::StreamExt as _;
use libp2p::{
    identify, identity, noise, ping, relay, rendezvous,
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, Swarm,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tracing::{debug, error, info, trace, warn};

/// Shared state exposed to the HTTP health handler.
#[derive(Clone)]
struct HealthState {
    peer_id: String,
    connections: Arc<AtomicUsize>,
}

/// Entry point for the bootstrap node.
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("decentgpu_bootstrap=debug".parse()?)
                .add_directive("libp2p=info".parse()?),
        )
        .json()
        .init();

    let cfg: BootstrapConfig = load_config("BOOTSTRAP").unwrap_or_default();

    info!(
        tcp_addr = %cfg.tcp_addr,
        quic_addr = %cfg.quic_addr,
        health_port = cfg.health_port,
        "bootstrap node starting"
    );

    // Read BOOTSTRAP_KEYPAIR_PATH directly — bypasses the config crate's
    // double-underscore separator so the common single-underscore env var works.
    let keypair_path = decentgpu_common::keypair::keypair_path_from_env(
        "BOOTSTRAP_KEYPAIR_PATH",
        "./bootstrap.keypair",
    );
    let keypair = decentgpu_common::keypair::load_or_generate(&keypair_path)
        .expect("failed to load or generate bootstrap keypair");
    let local_peer_id = identity::PeerId::from(keypair.public());
    info!(
        %local_peer_id,
        keypair_path = %keypair_path.display(),
        "bootstrap identity established"
    );

    let connections = Arc::new(AtomicUsize::new(0));

    let mut swarm = build_swarm(keypair).await?;

    let tcp_addr: Multiaddr = cfg.tcp_addr.parse().context("invalid tcp_addr")?;
    let quic_addr: Multiaddr = cfg.quic_addr.parse().context("invalid quic_addr")?;

    swarm.listen_on(tcp_addr).context("listen TCP failed")?;
    // QUIC is removed when WebSocket transport is in use (libp2p builder side effect).
    // Make it non-fatal so the node starts successfully with TCP + WebSocket only.
    if let Err(e) = swarm.listen_on(quic_addr.clone()) {
        warn!(addr = %quic_addr, err = %e, "QUIC listener unavailable (non-fatal, using TCP+WS)");
    }

    // WebSocket listener — for Cloudflare Tunnel / production
    let ws_port = std::env::var("BOOTSTRAP_WS_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(9002);
    let ws_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{ws_port}/ws")
        .parse()
        .context("invalid ws_addr")?;
    if let Err(e) = swarm.listen_on(ws_addr.clone()) {
        tracing::warn!(addr = %ws_addr, err = %e, "WebSocket listener failed (non-fatal)");
    } else {
        tracing::info!(addr = %ws_addr, "bootstrap listening on WebSocket");
    }

    // Spawn HTTP health endpoint in a separate task.
    let health_state = HealthState {
        peer_id: local_peer_id.to_string(),
        connections: Arc::clone(&connections),
    };
    let health_port = cfg.health_port;
    tokio::spawn(async move {
        serve_health(health_port, health_state).await;
    });

    run_event_loop(swarm, connections).await;

    info!("Bootstrap node shutting down");
    Ok(())
}


/// Build the libp2p Swarm with TCP + WebSocket + QUIC transports and all bootstrap behaviours.
async fn build_swarm(keypair: identity::Keypair) -> Result<Swarm<behaviour::BootstrapBehaviour>> {
    let local_peer_id = identity::PeerId::from(keypair.public());

    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )
        .context("TCP transport setup failed")?
        .with_websocket(noise::Config::new, yamux::Config::default)
        .await
        .context("WebSocket transport")?
        .with_behaviour(|key| {
            Ok(behaviour::BootstrapBehaviour {
                identify: identify::Behaviour::new(identify::Config::new(
                    "/decentgpu/identify/1.0.0".into(),
                    key.public(),
                )),
                // More lenient ping: 60s timeout, 30s interval, allow 5 consecutive
                // failures before the connection is considered dead.  The default
                // 20s timeout / 3 failures was dropping worker/master connections
                // that are otherwise healthy (BUG 4).
                ping: ping::Behaviour::new(
                    ping::Config::new()
                        .with_timeout(Duration::from_secs(60))
                        .with_interval(Duration::from_secs(30)),
                ),
                rendezvous: rendezvous::server::Behaviour::new(
                    rendezvous::server::Config::default(),
                ),
                relay: relay::Behaviour::new(local_peer_id, relay::Config::default()),
            })
        })
        .context("behaviour construction failed")?
        // Keep connections open for 2 hours — rendezvous registrations are valid for
        // 7200 s and workers/master must stay connected that long (BUG 4).
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(7200)))
        .build();

    Ok(swarm)
}

/// Drive the swarm event loop until a shutdown signal is received.
async fn run_event_loop(
    mut swarm: Swarm<behaviour::BootstrapBehaviour>,
    connections: Arc<AtomicUsize>,
) {
    let mut shutdown = std::pin::pin!(shutdown_signal());

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &connections);
            }
            _ = &mut shutdown => {
                break;
            }
        }
    }
}

/// Handle every known swarm event variant. `SwarmEvent` is `#[non_exhaustive]`
/// so a wildcard arm is required by the compiler for future-proofing.
fn handle_swarm_event(
    event: SwarmEvent<behaviour::BootstrapBehaviourEvent>,
    connections: &Arc<AtomicUsize>,
) {
    match event {
        // ── Transport / connection lifecycle ──────────────────────────────
        SwarmEvent::NewListenAddr { address, .. } => {
            info!(address = %address, "listening on new address");
        }
        SwarmEvent::ListenerClosed { addresses, reason, .. } => {
            warn!(addresses = ?addresses, reason = ?reason, "listener closed");
        }
        SwarmEvent::ListenerError { error, .. } => {
            error!(error = %error, "listener error");
        }
        SwarmEvent::ConnectionEstablished {
            peer_id,
            endpoint,
            num_established,
            ..
        } => {
            connections.fetch_add(1, Ordering::Relaxed);
            info!(
                peer_id = %peer_id,
                endpoint = ?endpoint,
                total = num_established,
                "connection established"
            );
        }
        SwarmEvent::ConnectionClosed {
            peer_id,
            cause,
            num_established,
            ..
        } => {
            // Saturating sub avoids underflow if events arrive out of order.
            connections
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    Some(v.saturating_sub(1))
                })
                .ok();
            info!(
                peer_id = %peer_id,
                cause = ?cause,
                remaining = num_established,
                "connection closed"
            );
        }
        SwarmEvent::IncomingConnection {
            local_addr,
            send_back_addr,
            ..
        } => {
            debug!(
                local_addr = %local_addr,
                send_back_addr = %send_back_addr,
                "incoming connection attempt"
            );
        }
        SwarmEvent::IncomingConnectionError {
            local_addr,
            send_back_addr,
            error,
            ..
        } => {
            warn!(
                local_addr = %local_addr,
                send_back_addr = %send_back_addr,
                error = %error,
                "incoming connection error"
            );
        }
        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            warn!(
                peer_id = ?peer_id,
                error = %error,
                "outgoing connection error"
            );
        }
        SwarmEvent::NewExternalAddrCandidate { address } => {
            debug!(address = %address, "new external address candidate");
        }
        SwarmEvent::ExternalAddrConfirmed { address } => {
            info!(address = %address, "external address confirmed");
        }
        SwarmEvent::ExternalAddrExpired { address } => {
            debug!(address = %address, "external address expired");
        }
        SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
            debug!(
                peer_id = %peer_id,
                address = %address,
                "new external address of peer"
            );
        }
        SwarmEvent::ExpiredListenAddr { address, .. } => {
            debug!(address = %address, "listen address expired");
        }
        // ── Behaviour events ──────────────────────────────────────────────
        SwarmEvent::Behaviour(event) => handle_behaviour_event(event),
        // SwarmEvent is #[non_exhaustive] — log unknown variants rather than silently ignoring.
        unknown => {
            debug!(event = ?unknown, "unhandled swarm event (future libp2p variant)");
        }
    }
}

/// Handle behaviour-specific events with full structured logging.
#[allow(deprecated)] // relay has deprecated variants pending removal per issue #4757
fn handle_behaviour_event(event: behaviour::BootstrapBehaviourEvent) {
    match event {
        // ── Rendezvous server ─────────────────────────────────────────────
        behaviour::BootstrapBehaviourEvent::Rendezvous(
            rendezvous::server::Event::PeerRegistered { peer, registration },
        ) => {
            info!(
                peer_id = %peer,
                namespace = %registration.namespace,
                "rendezvous: peer registered"
            );
        }
        behaviour::BootstrapBehaviourEvent::Rendezvous(
            rendezvous::server::Event::PeerNotRegistered {
                peer,
                namespace,
                error,
            },
        ) => {
            warn!(
                peer_id = %peer,
                namespace = %namespace,
                error = ?error,
                "rendezvous: peer registration rejected"
            );
        }
        behaviour::BootstrapBehaviourEvent::Rendezvous(
            rendezvous::server::Event::PeerUnregistered { peer, namespace },
        ) => {
            info!(
                peer_id = %peer,
                namespace = %namespace,
                "rendezvous: peer unregistered"
            );
        }
        behaviour::BootstrapBehaviourEvent::Rendezvous(
            rendezvous::server::Event::DiscoverServed {
                enquirer,
                registrations,
            },
        ) => {
            debug!(
                enquirer = %enquirer,
                count = registrations.len(),
                "rendezvous: discover request served"
            );
        }
        behaviour::BootstrapBehaviourEvent::Rendezvous(
            rendezvous::server::Event::DiscoverNotServed { enquirer, error },
        ) => {
            warn!(
                enquirer = %enquirer,
                error = ?error,
                "rendezvous: discover request not served"
            );
        }
        behaviour::BootstrapBehaviourEvent::Rendezvous(
            rendezvous::server::Event::RegistrationExpired(registration),
        ) => {
            debug!(
                peer_id = %registration.record.peer_id(),
                namespace = %registration.namespace,
                "rendezvous: registration expired"
            );
        }

        // ── Relay v2 server (non-deprecated variants) ─────────────────────
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::ReservationReqAccepted {
            src_peer_id,
            renewed,
        }) => {
            info!(
                src_peer_id = %src_peer_id,
                renewed = renewed,
                "relay: reservation accepted"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::ReservationReqDenied {
            src_peer_id,
        }) => {
            warn!(src_peer_id = %src_peer_id, "relay: reservation denied");
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::ReservationTimedOut {
            src_peer_id,
        }) => {
            debug!(src_peer_id = %src_peer_id, "relay: reservation timed out");
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::CircuitReqAccepted {
            src_peer_id,
            dst_peer_id,
        }) => {
            info!(
                src_peer_id = %src_peer_id,
                dst_peer_id = %dst_peer_id,
                "relay: circuit accepted"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::CircuitReqDenied {
            src_peer_id,
            dst_peer_id,
        }) => {
            warn!(
                src_peer_id = %src_peer_id,
                dst_peer_id = %dst_peer_id,
                "relay: circuit denied"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::CircuitClosed {
            src_peer_id,
            dst_peer_id,
            error,
        }) => {
            debug!(
                src_peer_id = %src_peer_id,
                dst_peer_id = %dst_peer_id,
                error = ?error,
                "relay: circuit closed"
            );
        }

        // ── Relay v2 server (deprecated variants — kept for exhaustive match) ──
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::ReservationReqAcceptFailed {
            src_peer_id,
            error,
        }) => {
            warn!(
                src_peer_id = %src_peer_id,
                error = ?error,
                "relay: reservation accept failed"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::ReservationReqDenyFailed {
            src_peer_id,
            error,
        }) => {
            warn!(
                src_peer_id = %src_peer_id,
                error = ?error,
                "relay: reservation deny failed"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::CircuitReqDenyFailed {
            src_peer_id,
            dst_peer_id,
            error,
        }) => {
            warn!(
                src_peer_id = %src_peer_id,
                dst_peer_id = %dst_peer_id,
                error = ?error,
                "relay: circuit deny failed"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(
            relay::Event::CircuitReqOutboundConnectFailed {
                src_peer_id,
                dst_peer_id,
                error,
            },
        ) => {
            warn!(
                src_peer_id = %src_peer_id,
                dst_peer_id = %dst_peer_id,
                error = ?error,
                "relay: circuit outbound connect failed"
            );
        }
        behaviour::BootstrapBehaviourEvent::Relay(relay::Event::CircuitReqAcceptFailed {
            src_peer_id,
            dst_peer_id,
            error,
        }) => {
            warn!(
                src_peer_id = %src_peer_id,
                dst_peer_id = %dst_peer_id,
                error = ?error,
                "relay: circuit accept failed"
            );
        }

        // ── Identify ──────────────────────────────────────────────────────
        behaviour::BootstrapBehaviourEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        }) => {
            debug!(
                peer_id = %peer_id,
                agent_version = %info.agent_version,
                protocol_version = %info.protocol_version,
                "identify: info received"
            );
        }
        behaviour::BootstrapBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. }) => {
            debug!(peer_id = %peer_id, "identify: info sent to peer");
        }
        behaviour::BootstrapBehaviourEvent::Identify(identify::Event::Pushed {
            peer_id, ..
        }) => {
            debug!(peer_id = %peer_id, "identify: info pushed to peer");
        }
        behaviour::BootstrapBehaviourEvent::Identify(identify::Event::Error {
            peer_id,
            error,
            ..
        }) => {
            warn!(peer_id = %peer_id, error = %error, "identify: error");
        }

        // ── Ping (trace level — do not spam logs) ─────────────────────────
        behaviour::BootstrapBehaviourEvent::Ping(ping::Event {
            peer,
            result: Ok(rtt),
            ..
        }) => {
            trace!(peer_id = %peer, rtt_ms = rtt.as_millis(), "ping: ok");
        }
        behaviour::BootstrapBehaviourEvent::Ping(ping::Event {
            peer,
            result: Err(ref e),
            ..
        }) => {
            debug!(peer_id = %peer, error = %e, "ping: failure");
        }
    }
}

/// Wait for SIGTERM or SIGINT before returning.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

/// Serve the HTTP health endpoint on a dedicated port using axum.
async fn serve_health(port: u16, state: HealthState) {
    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(addr = %addr, error = %e, "failed to bind health endpoint");
            return;
        }
    };

    info!(addr = %addr, "health endpoint listening");
    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "health endpoint error");
    }
}

/// `GET /health` — returns JSON with status, peer_id, and live connection count.
async fn health_handler(State(state): State<HealthState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "bootstrap",
        "peer_id": state.peer_id,
        "connections": state.connections.load(Ordering::Relaxed),
    }))
}
