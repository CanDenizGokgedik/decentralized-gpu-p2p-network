//! Worker P2P event loop — registers with bootstrap, publishes heartbeats, accepts job assignments.

use std::time::{Duration, Instant};

use anyhow::Result;
use decentgpu_common::types::WorkerCapabilities;
use decentgpu_proto::{
    envelope::Payload, Envelope, GpuBackend as ProtoGpuBackend, GpuInfo as ProtoGpuInfo, HeartBeat,
    JobAck, JobAssignment, RegisterWorker, RegisterWorkerAck, WorkerCapabilities as ProtoCapabilities,
};
use futures::StreamExt as _;
use libp2p::{
    autonat, dcutr, gossipsub, identify,
    kad::QueryResult,
    ping, relay, rendezvous,
    request_response,
    swarm::{dial_opts::DialOpts, SwarmEvent},
    Multiaddr, PeerId, Swarm,
};
use prost::Message as _;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

use super::behaviour::{job_codec, WorkerBehaviour, WorkerBehaviourEvent, TOPIC_HEARTBEAT};

const RENDEZVOUS_NAMESPACE: &str = "workers";
// The rendezvous server (libp2p-rendezvous 0.15) enforces a minimum TTL of 7200 s (2 h).
// Using 3600 causes an InvalidTtl rejection.
const RENDEZVOUS_TTL: u64 = 7200;
const RECONNECT_DELAY: Duration = Duration::from_secs(10);
// Safe far-future sentinel — u64::MAX/2 overflows Instant on macOS.
const FAR_FUTURE_SECS: u64 = 86_400 * 365 * 10; // 10 years

pub struct JobResult {
    pub job_id: String,
    pub success: bool,
    pub output: String,
}

pub struct WorkerEventLoop {
    swarm: Swarm<WorkerBehaviour>,
    bootstrap_addr: Multiaddr,
    bootstrap_peer_id: Option<PeerId>,
    heartbeat_topic: gossipsub::IdentTopic,
    capabilities: WorkerCapabilities,
    job_result_rx: mpsc::Receiver<JobResult>,
    job_tx: mpsc::Sender<JobAssignment>,
    started_at: Instant,
    /// Optional JWT auth token for linking this worker to a user account.
    auth_token: Option<String>,
    /// Whether registration has been attempted for the current bootstrap peer.
    registration_attempted: bool,
    /// Optional direct multiaddr for the master node. When set the worker dials
    /// the master directly after a successful rendezvous registration.
    master_addr: Option<String>,
    /// PeerId of the master once we've identified it — used to send JobCompleted.
    master_peer_id: Option<PeerId>,
}

impl WorkerEventLoop {
    pub fn new(
        swarm: Swarm<WorkerBehaviour>,
        bootstrap_addr: Multiaddr,
        capabilities: WorkerCapabilities,
        job_tx: mpsc::Sender<JobAssignment>,
        job_result_rx: mpsc::Receiver<JobResult>,
        auth_token: Option<String>,
        master_addr: Option<String>,
    ) -> Self {
        Self {
            swarm,
            bootstrap_addr,
            bootstrap_peer_id: None,
            heartbeat_topic: gossipsub::IdentTopic::new(TOPIC_HEARTBEAT),
            capabilities,
            job_result_rx,
            job_tx,
            started_at: Instant::now(),
            auth_token,
            registration_attempted: false,
            master_addr,
            master_peer_id: None,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let reregister = tokio::time::sleep(Duration::from_secs(FAR_FUTURE_SECS));
        tokio::pin!(reregister);
        let reconnect = tokio::time::sleep(Duration::from_secs(FAR_FUTURE_SECS));
        tokio::pin!(reconnect);
        let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut shutdown = std::pin::pin!(shutdown_signal());

        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event, &mut reregister, &mut reconnect).await;
                }
                _ = heartbeat.tick() => { self.publish_heartbeat(); }
                Some(result) = self.job_result_rx.recv() => {
                    // Send JobCompleted to master via request-response.
                    if let Some(master_id) = self.master_peer_id {
                        let completed = decentgpu_proto::JobCompleted {
                            job_id:            result.job_id.clone(),
                            success:           result.success,
                            error_message:     if result.success { String::new() } else { result.output.clone() },
                            duration_secs:     0,
                            result_size_bytes: result.output.len() as u64,
                            // Send full output so master can persist it for terminal log replay.
                            output:            result.output.clone(),
                        };
                        let env = Envelope {
                            payload: Some(Payload::JobCompleted(completed)),
                        };
                        let req_bytes = env.encode_to_vec();
                        info!(
                            job_id = %result.job_id,
                            success = result.success,
                            "sending JobCompleted to master"
                        );
                        self.swarm.behaviour_mut().job_rr.send_request(
                            &master_id,
                            job_codec::JobRequest(req_bytes),
                        );
                    } else {
                        warn!(job_id = %result.job_id, "master_peer_id not set, cannot send JobCompleted");
                    }
                }
                _ = &mut reregister => {
                    self.do_register();
                    reregister.as_mut().reset(
                        tokio::time::Instant::now() + Duration::from_secs(RENDEZVOUS_TTL * 9 / 10),
                    );
                }
                _ = &mut reconnect => {
                    info!("reconnecting to bootstrap");
                    if let Err(e) = self.swarm.dial(
                        DialOpts::unknown_peer_id()
                            .address(self.bootstrap_addr.clone())
                            .allocate_new_port()
                            .build(),
                    ) {
                        warn!(error = %e, "reconnect dial failed");
                    }
                    reconnect.as_mut().reset(tokio::time::Instant::now() + RECONNECT_DELAY);
                }
                _ = &mut shutdown => {
                    info!("shutdown signal — stopping worker P2P");
                    break;
                }
            }
        }
        Ok(())
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<WorkerBehaviourEvent>,
        reregister: &mut std::pin::Pin<&mut tokio::time::Sleep>,
        reconnect: &mut std::pin::Pin<&mut tokio::time::Sleep>,
    ) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(addr = %address, "worker listening");
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                use libp2p::core::ConnectedPoint;
                if matches!(endpoint, ConnectedPoint::Dialer { .. }) && self.bootstrap_peer_id.is_none() {
                    info!(peer_id = %peer_id, "outbound connection established (likely bootstrap)");
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                if Some(peer_id) == self.bootstrap_peer_id {
                    warn!(peer_id = %peer_id, "bootstrap connection closed — will reconnect");
                    self.bootstrap_peer_id = None;
                    self.registration_attempted = false;
                    reconnect.as_mut().reset(tokio::time::Instant::now() + RECONNECT_DELAY);
                }
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!(peer_id = ?peer_id, error = %error, "outgoing connection error");
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                info!(addr = %address, "external address confirmed — retrying rendezvous");
                self.do_register();
            }
            SwarmEvent::Behaviour(ev) => {
                self.handle_behaviour_event(ev, reregister).await;
            }
            _ => {}
        }
    }

    async fn handle_behaviour_event(
        &mut self,
        event: WorkerBehaviourEvent,
        reregister: &mut std::pin::Pin<&mut tokio::time::Sleep>,
    ) {
        use WorkerBehaviourEvent as E;
        match event {
            E::Kademlia(ev) => match ev {
                libp2p::kad::Event::RoutingUpdated { peer, .. } => {
                    trace!(peer_id = %peer, "kademlia routing updated");
                }
                libp2p::kad::Event::OutboundQueryProgressed { result, .. } => match result {
                    QueryResult::Bootstrap(Ok(b)) => {
                        if b.num_remaining == 0 { info!("kademlia bootstrap complete"); }
                    }
                    QueryResult::Bootstrap(Err(e)) => {
                        warn!(error = ?e, "kademlia bootstrap error");
                    }
                    _ => {}
                },
                _ => {}
            },

            E::Rendezvous(rendezvous::client::Event::Registered { namespace, ttl, rendezvous_node }) => {
                info!(namespace = %namespace, ttl, node = %rendezvous_node, "registered with rendezvous");
                reregister.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(ttl * 9 / 10));

                // After successful rendezvous registration, dial the master directly if configured
                // and not already connected (avoid redundant re-dials causing connection churn).
                let already_connected = self.master_peer_id
                    .map(|pid| self.swarm.is_connected(&pid))
                    .unwrap_or(false);
                if already_connected {
                    debug!("already connected to master, skipping re-dial after rendezvous registration");
                }
                if let Some(addr_str) = if already_connected { None } else { self.master_addr.clone() } {
                    match addr_str.parse::<Multiaddr>() {
                        Ok(addr) => {
                            // Guard: 0.0.0.0 is a listen wildcard — not a valid dial target.
                            // Workers must be given 127.0.0.1 (local) or the real server IP.
                            if addr.to_string().contains("/ip4/0.0.0.0") {
                                tracing::error!(
                                    addr = %addr,
                                    "WORKER_MASTER_ADDR contains 0.0.0.0 — invalid for dialing. \
                                     Use 127.0.0.1 for local or the actual server IP for remote."
                                );
                            } else {
                                // Use allocate_new_port() so libp2p does NOT try to
                                // reuse the worker's listen port when connecting to the
                                // master — avoids "Address already in use (os error 48)"
                                // on macOS when both endpoints are on 127.0.0.1.
                                match self.swarm.dial(
                                    libp2p::swarm::dial_opts::DialOpts::unknown_peer_id()
                                        .address(addr.clone())
                                        .allocate_new_port()
                                        .build(),
                                ) {
                                    Ok(_) => info!(addr = %addr, "dialing master directly after rendezvous registration"),
                                    Err(e) => warn!(error = %e, "failed to dial master directly"),
                                }
                            }
                        }
                        Err(e) => warn!(error = %e, addr = %addr_str, "invalid master addr"),
                    }
                }
            }
            E::Rendezvous(rendezvous::client::Event::RegisterFailed { namespace, error, rendezvous_node }) => {
                warn!(namespace = %namespace, node = %rendezvous_node, error = ?error, "rendezvous registration failed");
            }
            E::Rendezvous(rendezvous::client::Event::Expired { peer }) => {
                info!(peer_id = %peer, "rendezvous registration expired — re-registering");
                self.do_register();
            }
            E::Rendezvous(_) => {}

            E::Gossipsub(_) => {}  // Workers publish only; no consume needed.

            // ── Job assignment via request-response ────────────────────────
            E::JobRr(request_response::Event::Message { peer, message }) => {
                match message {
                    request_response::Message::Request { request, channel, .. } => {
                        // Decode the JobAssignment.
                        match JobAssignment::decode(request.0.as_slice()) {
                            Ok(assignment) => {
                                let job_id = assignment.spec.as_ref()
                                    .map(|s| s.job_id.clone())
                                    .unwrap_or_default();
                                info!(peer = %peer, job_id = %job_id, "received job assignment");

                                // Send to executor.
                                let accepted = self.job_tx.try_send(assignment).is_ok();

                                let ack = JobAck {
                                    job_id,
                                    accepted,
                                    reason: if accepted {
                                        String::new()
                                    } else {
                                        "executor queue full".into()
                                    },
                                };
                                let _ = self.swarm.behaviour_mut().job_rr.send_response(
                                    channel,
                                    job_codec::JobResponse(ack.encode_to_vec()),
                                );
                            }
                            Err(e) => {
                                warn!(peer = %peer, error = %e, "failed to decode JobAssignment");
                                let ack = JobAck {
                                    job_id: String::new(),
                                    accepted: false,
                                    reason: format!("decode error: {e}"),
                                };
                                let _ = self.swarm.behaviour_mut().job_rr.send_response(
                                    channel,
                                    job_codec::JobResponse(ack.encode_to_vec()),
                                );
                            }
                        }
                    }
                    request_response::Message::Response { response, .. } => {
                        // Response from master — could be either:
                        //  • RegisterWorkerAck (in an Envelope) — reply to RegisterWorker
                        //  • JobAck (plain proto)               — reply to JobCompleted
                        // Try Envelope first; fall back to plain JobAck.
                        self.handle_master_response(response.0.as_slice());
                    }
                }
            }
            E::JobRr(request_response::Event::InboundFailure { peer, error, .. }) => {
                warn!(peer = %peer, error = %error, "job rr inbound failure");
            }
            E::JobRr(request_response::Event::OutboundFailure { peer, error, .. }) => {
                warn!(peer = %peer, error = %error, "registration request outbound failure");
            }
            E::JobRr(_) => {}

            E::Identify(identify::Event::Received { peer_id, info, .. }) => {
                debug!(peer_id = %peer_id, agent = %info.agent_version, "identify received");

                // Add the observed external address reported by the remote peer before
                // attempting rendezvous registration, so the rendezvous server sees at
                // least one externally reachable address.
                {
                    let observed: Multiaddr = info.observed_addr.clone();
                    self.swarm.add_external_address(observed.clone());
                    debug!(addr = %observed, "learned external addr from identify peer");
                }

                if self.bootstrap_peer_id.is_none() {
                    let addr = info.listen_addrs.first().cloned()
                        .unwrap_or_else(|| self.bootstrap_addr.clone());
                    self.on_bootstrap_identified(peer_id, addr);
                }
                for addr in &info.listen_addrs {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                }

                // If this is the master node (identified by agent string), track it and register.
                if info.agent_version.starts_with("master/") {
                    self.master_peer_id = Some(peer_id);
                    if !self.registration_attempted {
                        info!(peer_id = %peer_id, "identified master — sending RegisterWorker");
                        self.try_register_with_master(peer_id);
                    }
                }
            }
            E::Identify(identify::Event::Sent { peer_id, .. }) => {
                trace!(peer_id = %peer_id, "identify sent");
            }
            E::Identify(identify::Event::Pushed { peer_id, .. }) => {
                trace!(peer_id = %peer_id, "identify pushed");
            }
            E::Identify(identify::Event::Error { peer_id, error, .. }) => {
                warn!(peer_id = %peer_id, error = %error, "identify error");
            }

            E::Relay(relay::client::Event::ReservationReqAccepted { relay_peer_id, renewal, .. }) => {
                info!(relay_peer_id = %relay_peer_id, renewed = renewal, "relay reservation accepted");
            }
            E::Relay(_) => {}

            E::Dcutr(dcutr::Event { remote_peer_id, result }) => match result {
                Ok(_) => info!(peer_id = %remote_peer_id, "NAT hole punch succeeded"),
                Err(e) => warn!(peer_id = %remote_peer_id, error = %e, "NAT hole punch failed"),
            },

            E::Autonat(autonat::Event::StatusChanged { old, new }) => {
                info!(old = ?old, new = ?new, "NAT status changed");
            }
            E::Autonat(_) => {}

            E::Ping(ping::Event { peer, result: Ok(rtt), .. }) => {
                trace!(peer_id = %peer, rtt_ms = rtt.as_millis(), "ping ok");
            }
            E::Ping(ping::Event { peer, result: Err(ref e), .. }) => {
                debug!(peer_id = %peer, error = %e, "ping failure");
            }
        }
    }

    fn on_bootstrap_identified(&mut self, peer_id: PeerId, addr: Multiaddr) {
        info!(peer_id = %peer_id, addr = %addr, "bootstrap identified — registering");
        self.bootstrap_peer_id = Some(peer_id);
        self.swarm.behaviour_mut().autonat.add_server(peer_id, Some(addr.clone()));
        self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
        self.do_register();
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            warn!(error = ?e, "kademlia bootstrap error");
        }
        // Note: RegisterWorker is sent to the master (not bootstrap) once we
        // identify the master via identify::Event::Received with agent "master/*".
    }

    /// Attempt to send a RegisterWorker message to the master via request-response.
    fn try_register_with_master(&mut self, master_peer_id: PeerId) {
        if self.registration_attempted {
            return;
        }
        let token = match &self.auth_token {
            Some(t) => t.clone(),
            None => return, // No token — skip registration.
        };

        self.registration_attempted = true;
        let local_peer_id = *self.swarm.local_peer_id();

        info!(master_peer_id = %master_peer_id, "sending RegisterWorker to master");

        let reg_msg = RegisterWorker {
            peer_id: local_peer_id.to_string(),
            jwt_token: token,
            capabilities: Some(capabilities_to_proto(&self.capabilities)),
        };
        let envelope = Envelope {
            payload: Some(Payload::RegisterWorker(reg_msg)),
        };
        let bytes = envelope.encode_to_vec();

        self.swarm.behaviour_mut().job_rr.send_request(
            &master_peer_id,
            job_codec::JobRequest(bytes),
        );
    }

    /// Dispatch an inbound response from the master over the request-response channel.
    ///
    /// Master sends two kinds of RR responses:
    /// 1. `Envelope { payload: RegisterWorkerAck }` — reply to `RegisterWorker`
    /// 2. Plain `JobAck`                            — reply to `JobCompleted`
    ///
    /// We try to decode as Envelope first; if that reveals a `RegisterWorkerAck` we
    /// handle it.  Any other successful envelope decode is logged and ignored.
    /// If Envelope decode fails we attempt a plain `JobAck` decode (acknowledgement
    /// of a `JobCompleted` notification — no further action needed).
    fn handle_master_response(&self, raw: &[u8]) {
        // ── Try Envelope ─────────────────────────────────────────────────────
        if let Ok(env) = Envelope::decode(raw) {
            match env.payload {
                Some(Payload::RegisterWorkerAck(ack)) => {
                    if ack.success {
                        info!(user_id = %ack.user_id, "worker successfully linked to user account");
                    } else {
                        warn!(error = %ack.error, "worker registration failed");
                    }
                }
                other => {
                    // e.g. the master wraps a future message type — just trace it.
                    debug!(payload = ?other, "received envelope response from master (not RegisterWorkerAck)");
                }
            }
            return;
        }

        // ── Try plain JobAck (reply to JobCompleted) ──────────────────────
        if let Ok(ack) = JobAck::decode(raw) {
            if ack.accepted {
                debug!(job_id = %ack.job_id, "master acknowledged JobCompleted");
            } else {
                warn!(job_id = %ack.job_id, reason = %ack.reason, "master rejected JobCompleted");
            }
            return;
        }

        warn!(bytes = raw.len(), "failed to decode any known response from master");
    }

    fn do_register(&mut self) {
        if let Some(bootstrap_peer_id) = self.bootstrap_peer_id {
            if let Err(e) = self.swarm.behaviour_mut().rendezvous.register(
                rendezvous::Namespace::from_static(RENDEZVOUS_NAMESPACE),
                bootstrap_peer_id,
                Some(RENDEZVOUS_TTL),
            ) {
                warn!(error = %e, "rendezvous register call failed");
            }
        }
    }

    fn publish_heartbeat(&mut self) {
        let uptime_secs = self.started_at.elapsed().as_secs();
        let uptime_percent = (uptime_secs as f32 / 86400.0 * 100.0).min(100.0);
        let hb = HeartBeat {
            peer_id: self.swarm.local_peer_id().to_string(),
            uptime_percent,
            is_busy: false,
            jobs_completed: 0,
        };
        let envelope = Envelope { payload: Some(Payload::HeartBeat(hb)) };
        let bytes = envelope.encode_to_vec();
        match self.swarm.behaviour_mut().gossipsub.publish(self.heartbeat_topic.clone(), bytes) {
            Ok(_) => debug!("heartbeat published"),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                trace!("heartbeat skipped — no gossipsub peers yet");
            }
            Err(e) => warn!(error = %e, "heartbeat publish failed"),
        }
    }
}

fn capabilities_to_proto(caps: &WorkerCapabilities) -> ProtoCapabilities {
    let backend_to_proto = |b: decentgpu_common::types::GpuBackend| match b {
        decentgpu_common::types::GpuBackend::Cuda    => ProtoGpuBackend::Cuda as i32,
        decentgpu_common::types::GpuBackend::Metal   => ProtoGpuBackend::Metal as i32,
        decentgpu_common::types::GpuBackend::Rocm    => ProtoGpuBackend::Rocm as i32,
        decentgpu_common::types::GpuBackend::CpuOnly => ProtoGpuBackend::CpuOnly as i32,
    };
    ProtoCapabilities {
        peer_id: String::new(),
        gpus: caps.gpus.iter().map(|g| ProtoGpuInfo {
            name: g.name.clone(),
            vram_mb: g.vram_mb,
            backend: backend_to_proto(g.backend),
        }).collect(),
        cpu: Some(decentgpu_proto::CpuInfo {
            model:    caps.cpu.model.clone(),
            cores:    caps.cpu.cores,
            threads:  caps.cpu.threads,
            freq_mhz: caps.cpu.freq_mhz,
        }),
        ram_mb:         caps.ram_mb,
        disk_mb:        caps.disk_mb,
        os:             caps.os.clone(),
        worker_version: caps.worker_version.clone(),
    }
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async { signal::ctrl_c().await.expect("Ctrl+C handler failed") };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler failed")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {} _ = terminate => {} }
}
