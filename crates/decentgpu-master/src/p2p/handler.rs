//! Master P2P event loop — connects to bootstrap, discovers workers, handles heartbeats.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use dashmap::DashMap;
use decentgpu_proto::{
    envelope::Payload, Envelope, HeartBeat, JobAck, JobAssignment, RegisterWorkerAck,
};
use futures::StreamExt as _;
use libp2p::{
    autonat, dcutr, gossipsub, identify,
    kad::{self, QueryResult},
    ping, relay, rendezvous,
    request_response,
    swarm::{dial_opts::DialOpts, SwarmEvent},
    Multiaddr, PeerId, Swarm,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use prost::Message as _;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use super::{
    behaviour::{job_codec, MasterBehaviour, MasterBehaviourEvent},
    protocols,
};

// The rendezvous server (libp2p-rendezvous 0.15) enforces a minimum TTL of 7200 s (2 h).
// Using 3600 causes an InvalidTtl rejection.
const RENDEZVOUS_TTL: u64 = 7200;
const RECONNECT_DELAY: Duration = Duration::from_secs(10);
// Safe far-future sentinel for tokio::time::sleep that does NOT overflow Instant on any platform.
// u64::MAX/2 nanoseconds (≈292 years) causes an overflow panic on macOS; 10 years is safe.
const FAR_FUTURE_SECS: u64 = 86_400 * 365 * 10; // 10 years

/// Guards against concurrent reconnect attempts.
static RECONNECTING: AtomicBool = AtomicBool::new(false);

/// A worker lifecycle event sent from the P2P layer to the API/scheduler.
#[derive(Debug)]
pub enum WorkerEvent {
    Online {
        peer_id: PeerId,
        capabilities: decentgpu_proto::WorkerCapabilities,
    },
    Offline { peer_id: PeerId },
    Heartbeat {
        peer_id: PeerId,
        uptime: f32,
        is_busy: bool,
        jobs_completed: u64,
    },
}

/// A job command sent from the API/scheduler to the P2P layer.
#[derive(Debug)]
pub enum JobCommand {
    Assign {
        peer_id: PeerId,
        job: Box<JobAssignment>,
    },
    Cancel { peer_id: PeerId, job_id: String },
}

/// Internal result of a worker registration attempt.
struct RegistrationResult {
    peer_id:      PeerId,
    ack:          RegisterWorkerAck,
    /// Capabilities from the RegisterWorker message (present on success, None on failure).
    capabilities: Option<decentgpu_proto::WorkerCapabilities>,
    response_channel: request_response::ResponseChannel<job_codec::JobResponse>,
}

pub struct MasterEventLoop {
    swarm: Swarm<MasterBehaviour>,
    bootstrap_addr: Multiaddr,
    bootstrap_peer_id: Option<PeerId>,
    heartbeat_topic: gossipsub::IdentTopic,
    worker_tx: mpsc::Sender<WorkerEvent>,
    job_rx: mpsc::Receiver<JobCommand>,
    started_at: Instant,
    /// Tracks consecutive failed bootstrap reconnect attempts for exponential backoff.
    reconnect_attempts: u32,
    /// JWT secret for verifying worker auth tokens.
    jwt_secret: Option<Vec<u8>>,
    /// Database for upserting worker records.
    db: Option<crate::db::Database>,
    /// Channel for receiving registration results from spawned tasks.
    reg_result_tx: mpsc::Sender<RegistrationResult>,
    reg_result_rx: mpsc::Receiver<RegistrationResult>,
    /// Live log broadcast bus: job_id → sender.
    log_bus: Option<Arc<DashMap<Uuid, broadcast::Sender<serde_json::Value>>>>,
}

impl MasterEventLoop {
    pub fn new(
        swarm: Swarm<MasterBehaviour>,
        bootstrap_addr: Multiaddr,
        worker_tx: mpsc::Sender<WorkerEvent>,
        job_rx: mpsc::Receiver<JobCommand>,
    ) -> Self {
        let (reg_result_tx, reg_result_rx) = mpsc::channel::<RegistrationResult>(32);
        Self {
            swarm,
            bootstrap_addr,
            bootstrap_peer_id: None,
            heartbeat_topic: gossipsub::IdentTopic::new(protocols::TOPIC_HEARTBEAT),
            worker_tx,
            job_rx,
            started_at: Instant::now(),
            reconnect_attempts: 0,
            jwt_secret: None,
            db: None,
            reg_result_tx,
            reg_result_rx,
            log_bus: None,
        }
    }

    /// Attach a JWT secret and database for worker registration handling.
    pub fn with_auth(mut self, jwt_secret: Vec<u8>, db: crate::db::Database) -> Self {
        self.jwt_secret = Some(jwt_secret);
        self.db = Some(db);
        self
    }

    /// Attach a live log broadcast bus for terminal streaming.
    pub fn with_log_bus(mut self, log_bus: Arc<DashMap<Uuid, broadcast::Sender<serde_json::Value>>>) -> Self {
        self.log_bus = Some(log_bus);
        self
    }

    pub async fn run(mut self) -> Result<()> {
        let reregister = tokio::time::sleep(Duration::from_secs(FAR_FUTURE_SECS));
        tokio::pin!(reregister);
        let reconnect = tokio::time::sleep(Duration::from_secs(FAR_FUTURE_SECS));
        tokio::pin!(reconnect);
        let mut shutdown = std::pin::pin!(shutdown_signal());

        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event, reregister.as_mut(), reconnect.as_mut());
                }
                cmd = self.job_rx.recv() => {
                    match cmd {
                        Some(c) => self.handle_job_command(c),
                        None => break,
                    }
                }
                Some(reg_result) = self.reg_result_rx.recv() => {
                    let success = reg_result.ack.success;
                    let caps    = reg_result.capabilities.clone();

                    // Send the registration ack back to the worker.
                    let ack_env = Envelope {
                        payload: Some(Payload::RegisterWorkerAck(reg_result.ack)),
                    };
                    let ack_bytes = ack_env.encode_to_vec();
                    let _ = self.swarm.behaviour_mut().job_rr.send_response(
                        reg_result.response_channel,
                        job_codec::JobResponse(ack_bytes),
                    );
                    debug!(peer_id = %reg_result.peer_id, "sent RegisterWorkerAck");

                    // Notify the scheduler that this worker is now online so it can
                    // be considered for job assignment.
                    if success {
                        let caps_proto = caps.unwrap_or_default();
                        info!(peer_id = %reg_result.peer_id, "emitting WorkerEvent::Online to scheduler");
                        let _ = self.worker_tx.send(WorkerEvent::Online {
                            peer_id:      reg_result.peer_id,
                            capabilities: caps_proto,
                        }).await;
                    }
                }
                _ = &mut reregister => {
                    self.do_reregister();
                    reregister.as_mut().reset(
                        tokio::time::Instant::now() + Duration::from_secs(RENDEZVOUS_TTL * 9 / 10),
                    );
                }
                _ = &mut reconnect => {
                    // Guard: only allow one in-flight reconnect attempt at a time.
                    if RECONNECTING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                        // A reconnect is already pending; push timer out and wait.
                        reconnect.as_mut().reset(tokio::time::Instant::now() + RECONNECT_DELAY);
                    } else {
                        let delay_secs = std::cmp::min(30, 2u64.pow(self.reconnect_attempts.min(4)));
                        info!(attempt = self.reconnect_attempts + 1, delay_secs, "attempting bootstrap reconnect");
                        let dial_opts = DialOpts::unknown_peer_id()
                            .address(self.bootstrap_addr.clone())
                            .allocate_new_port()
                            .build();
                        match self.swarm.dial(dial_opts) {
                            Ok(()) => {
                                self.reconnect_attempts += 1;
                                reconnect.as_mut().reset(
                                    tokio::time::Instant::now() + Duration::from_secs(FAR_FUTURE_SECS),
                                );
                            }
                            Err(e) => {
                                warn!(error = %e, "bootstrap reconnect dial rejected");
                                RECONNECTING.store(false, Ordering::SeqCst);
                                reconnect.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(delay_secs));
                            }
                        }
                    }
                }
                _ = &mut shutdown => {
                    info!("Master P2P event loop shutting down");
                    break;
                }
            }
        }
        Ok(())
    }

    // ── Bootstrap helpers ─────────────────────────────────────────────────

    fn on_bootstrap_identified(&mut self, peer_id: PeerId, addr: Multiaddr) {
        self.bootstrap_peer_id = Some(peer_id);
        self.swarm.behaviour_mut().autonat.add_server(peer_id, Some(addr.clone()));
        self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);

        if let Err(e) = self.swarm.behaviour_mut().rendezvous.register(
            rendezvous::Namespace::from_static(protocols::NAMESPACE_MASTER),
            peer_id,
            Some(RENDEZVOUS_TTL),
        ) {
            error!(error = %e, "rendezvous register failed");
        }

        self.swarm.behaviour_mut().rendezvous.discover(
            Some(rendezvous::Namespace::from_static(protocols::NAMESPACE_WORKERS)),
            None,
            None,
            peer_id,
        );

        match self.swarm.behaviour_mut().kademlia.bootstrap() {
            Ok(_) => debug!("kademlia bootstrap query started"),
            Err(e) => warn!(error = ?e, "kademlia bootstrap failed"),
        }
    }

    fn do_reregister(&mut self) {
        if let Some(bootstrap_id) = self.bootstrap_peer_id {
            if let Err(e) = self.swarm.behaviour_mut().rendezvous.register(
                rendezvous::Namespace::from_static(protocols::NAMESPACE_MASTER),
                bootstrap_id,
                Some(RENDEZVOUS_TTL),
            ) {
                warn!(error = %e, "rendezvous re-registration failed");
            }
        }
    }

    // ── Job command handler ───────────────────────────────────────────────

    fn handle_job_command(&mut self, cmd: JobCommand) {
        match cmd {
            JobCommand::Assign { peer_id, job } => {
                let job_id = job.spec.as_ref().map(|s| s.job_id.as_str()).unwrap_or("?");
                info!(peer_id = %peer_id, job_id, "sending job assignment via request-response");
                let req_bytes = job_codec::encode(&*job);
                self.swarm
                    .behaviour_mut()
                    .job_rr
                    .send_request(&peer_id, super::behaviour::job_codec::JobRequest(req_bytes));
            }
            JobCommand::Cancel { peer_id, job_id } => {
                debug!(peer_id = %peer_id, job_id = %job_id, "job cancel command (stub)");
            }
        }
    }

    // ── Swarm event dispatch ──────────────────────────────────────────────

    fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<MasterBehaviourEvent>,
        reregister: std::pin::Pin<&mut tokio::time::Sleep>,
        reconnect: std::pin::Pin<&mut tokio::time::Sleep>,
    ) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(address = %address, "master p2p listening");
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, num_established, .. } => {
                info!(peer_id = %peer_id, direction = ?endpoint, connections = num_established, "connection established");
                if self.bootstrap_peer_id.is_none() {
                    if matches!(endpoint, libp2p::core::ConnectedPoint::Dialer { .. }) {
                        info!(peer_id = %peer_id, "connected to bootstrap");
                        self.reconnect_attempts = 0;
                        RECONNECTING.store(false, Ordering::SeqCst);
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, .. } => {
                info!(peer_id = %peer_id, cause = ?cause, remaining = num_established, "connection closed");
                if Some(peer_id) == self.bootstrap_peer_id && num_established == 0 {
                    let delay_secs = std::cmp::min(30, 2u64.pow(self.reconnect_attempts.min(4)));
                    warn!(peer_id = %peer_id, delay_secs, "lost bootstrap connection, scheduling reconnect");
                    RECONNECTING.store(false, Ordering::SeqCst);
                    reconnect.reset(tokio::time::Instant::now() + Duration::from_secs(delay_secs));
                }
                if Some(peer_id) != self.bootstrap_peer_id {
                    let _ = self.worker_tx.try_send(WorkerEvent::Offline { peer_id });
                }
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!(error = %error, "incoming connection error");
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!(peer_id = ?peer_id, error = %error, "outgoing connection error");
                if peer_id.is_none() || peer_id == self.bootstrap_peer_id {
                    let delay_secs = std::cmp::min(30, 2u64.pow(self.reconnect_attempts.min(4)));
                    RECONNECTING.store(false, Ordering::SeqCst);
                    reconnect.reset(tokio::time::Instant::now() + Duration::from_secs(delay_secs));
                }
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                debug!(peer_id = ?peer_id, "dialing peer");
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                info!(address = %address, "external address confirmed");
                self.do_reregister();
            }
            SwarmEvent::Behaviour(ev) => self.handle_behaviour(ev, reregister),
            _ => {}
        }
    }

    fn handle_behaviour(
        &mut self,
        event: MasterBehaviourEvent,
        reregister: std::pin::Pin<&mut tokio::time::Sleep>,
    ) {
        use MasterBehaviourEvent as E;
        match event {
            // ── Kademlia ─────────────────────────────────────────────────
            E::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
                match result {
                    QueryResult::Bootstrap(Ok(_)) => info!("kademlia bootstrap complete"),
                    QueryResult::Bootstrap(Err(e)) => warn!(error = ?e, "kademlia bootstrap error"),
                    _ => {}
                }
            }
            E::Kademlia(kad::Event::RoutingUpdated { peer, .. }) => {
                trace!(peer = %peer, "kademlia routing updated");
            }
            E::Kademlia(_) => {}

            // ── Rendezvous ───────────────────────────────────────────────
            E::Rendezvous(rendezvous::client::Event::Registered { namespace, ttl, .. }) => {
                info!(namespace = %namespace, ttl, "registered with rendezvous");
                reregister.reset(tokio::time::Instant::now() + Duration::from_secs(ttl * 9 / 10));
            }
            E::Rendezvous(rendezvous::client::Event::RegisterFailed { namespace, error, .. }) => {
                error!(namespace = %namespace, error = ?error, "rendezvous registration failed");
                reregister.reset(tokio::time::Instant::now() + Duration::from_secs(30));
            }
            E::Rendezvous(rendezvous::client::Event::Discovered { registrations, .. }) => {
                info!(count = registrations.len(), "discovered workers");
                for reg in &registrations {
                    let peer_id = reg.record.peer_id();
                    let addrs: Vec<Multiaddr> = reg.record.addresses().to_vec();
                    if let Err(e) = self.swarm.dial(
                        DialOpts::peer_id(peer_id).addresses(addrs).build(),
                    ) {
                        debug!(peer_id = %peer_id, error = %e, "dial worker");
                    }
                }
            }
            E::Rendezvous(rendezvous::client::Event::DiscoverFailed { namespace, error, .. }) => {
                warn!(namespace = ?namespace, error = ?error, "rendezvous discover failed");
            }
            E::Rendezvous(rendezvous::client::Event::Expired { peer }) => {
                debug!(peer = %peer, "rendezvous registration expired");
            }

            // ── Gossipsub ────────────────────────────────────────────────
            E::Gossipsub(gossipsub::Event::Message { propagation_source, message, .. }) => {
                self.handle_gossip_message(propagation_source, message);
            }
            E::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                debug!(peer_id = %peer_id, topic = %topic, "gossipsub: peer subscribed");
            }
            E::Gossipsub(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                debug!(peer_id = %peer_id, topic = %topic, "gossipsub: peer unsubscribed");
                let _ = self.worker_tx.try_send(WorkerEvent::Offline { peer_id });
            }
            E::Gossipsub(_) => {}

            // ── Request-response (job assignment / worker registration) ───
            E::JobRr(request_response::Event::Message { peer, message }) => {
                match message {
                    request_response::Message::Response { response, .. } => {
                        // Worker acknowledged the job assignment.
                        match JobAck::decode(response.0.as_slice()) {
                            Ok(ack) => {
                                if ack.accepted {
                                    info!(peer = %peer, job_id = %ack.job_id, "worker accepted job");
                                } else {
                                    warn!(peer = %peer, job_id = %ack.job_id,
                                        reason = %ack.reason, "worker rejected job");
                                }
                            }
                            Err(e) => warn!(peer = %peer, error = %e, "failed to decode JobAck"),
                        }
                    }
                    request_response::Message::Request { request, channel, .. } => {
                        // Decode the envelope to check the message type.
                        let raw = request.0.as_slice();
                        match Envelope::decode(raw) {
                            Ok(env) => {
                                match env.payload {
                                    Some(Payload::RegisterWorker(req)) => {
                                        // Spawn async task for DB work; send result back via channel.
                                        self.spawn_register_worker_task(peer, req, channel);
                                    }
                                    Some(Payload::JobCompleted(completed)) => {
                                        let job_id_str  = completed.job_id.clone();
                                        let success     = completed.success;
                                        let error_msg   = completed.error_message.clone();
                                        let output      = completed.output.clone();
                                        info!(
                                            peer = %peer,
                                            job_id = %job_id_str,
                                            success,
                                            output_len = output.len(),
                                            "received JobCompleted from worker"
                                        );
                                        // Spawn DB update + log storage + live broadcast.
                                        let db  = self.db.clone();
                                        let jid = job_id_str.clone();
                                        let log_bus_opt = self.log_bus.clone();
                                        let level_str: String = if success { "INFO".to_string() } else { "ERROR".to_string() };
                                        tokio::spawn(async move {
                                            if let Some(db) = db {
                                                if let Ok(job_id) = jid.parse::<uuid::Uuid>() {
                                                    // ── Store output lines BEFORE status transition ──
                                                    // This ensures terminal replay works even if the WS
                                                    // connects after the status changes to "completed".
                                                    let now_ms = std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .unwrap_or_default()
                                                        .as_millis() as i64;
                                                    let level = level_str.as_str();
                                                    for (i, line) in output.lines().enumerate() {
                                                        if !line.is_empty() {
                                                            // Offset each line by 1 ms to preserve order.
                                                            let ts = now_ms + i as i64;
                                                            if let Err(e) = db.jobs.append_log(job_id, ts, level, line).await {
                                                                warn!(%job_id, error = %e, "failed to append log line");
                                                            }
                                                        }
                                                    }
                                                    // ── Transition job status ────────────────────────
                                                    let new_status = if success { "completed" } else { "failed" };
                                                    let err_ref = if success { None } else { Some(error_msg.as_str()) };
                                                    match db.jobs.transition_status(
                                                        job_id,
                                                        "assigned",
                                                        new_status,
                                                        None,
                                                        err_ref,
                                                        None,
                                                    ).await {
                                                        Ok(_) => info!(%job_id, %new_status, "job status updated"),
                                                        Err(e) => warn!(%job_id, error = %e, "failed to update job status"),
                                                    }

                                                    // ── Broadcast to live terminal subscribers ───────
                                                    if let Some(ref log_bus) = log_bus_opt {
                                                        let mut line_idx = 0i64;
                                                        for line in output.lines() {
                                                            if line.is_empty() { continue; }
                                                            let ts = now_ms + line_idx;
                                                            line_idx += 1;
                                                            let msg = serde_json::json!({
                                                                "type":  "log",
                                                                "ts":    ts,
                                                                "level": level_str,
                                                                "data":  format!("{}\r\n", line),
                                                            });
                                                            if let Some(tx) = log_bus.get(&job_id) {
                                                                let _ = tx.send(msg);
                                                            }
                                                        }
                                                        // Signal job completion
                                                        let done_msg = serde_json::json!({ "type": "job_done", "status": new_status });
                                                        if let Some(tx) = log_bus.get(&job_id) {
                                                            let _ = tx.send(done_msg);
                                                        }
                                                        // Clean up the broadcast entry
                                                        log_bus.remove(&job_id);
                                                    }
                                                }
                                            }
                                        });
                                        // Send ack back.
                                        let ack = JobAck {
                                            job_id:   job_id_str,
                                            accepted: true,
                                            reason:   String::new(),
                                        };
                                        let _ = self.swarm.behaviour_mut().job_rr.send_response(
                                            channel,
                                            job_codec::JobResponse(job_codec::encode(&ack)),
                                        );
                                    }
                                    _ => {
                                        warn!(peer_id = %peer, "unexpected envelope type from peer");
                                        let ack = JobAck {
                                            job_id:   String::new(),
                                            accepted: false,
                                            reason:   "unexpected request type".into(),
                                        };
                                        let _ = self.swarm.behaviour_mut().job_rr.send_response(
                                            channel,
                                            job_codec::JobResponse(job_codec::encode(&ack)),
                                        );
                                    }
                                }
                            }
                            Err(_) => {
                                // Not an Envelope — unexpected plain message.
                                warn!(peer = %peer, "unexpected job request from peer (not an envelope)");
                                let ack = JobAck {
                                    job_id:   String::new(),
                                    accepted: false,
                                    reason:   "master does not accept requests".into(),
                                };
                                let _ = self.swarm.behaviour_mut().job_rr.send_response(
                                    channel,
                                    job_codec::JobResponse(job_codec::encode(&ack)),
                                );
                            }
                        }
                    }
                }
            }
            E::JobRr(request_response::Event::OutboundFailure { peer, error, .. }) => {
                warn!(peer = %peer, error = %error, "job assignment outbound failure");
            }
            E::JobRr(request_response::Event::InboundFailure { peer, error, .. }) => {
                warn!(peer = %peer, error = %error, "job assignment inbound failure");
            }
            E::JobRr(request_response::Event::ResponseSent { .. }) => {}

            // ── Identify ─────────────────────────────────────────────────
            E::Identify(identify::Event::Received { peer_id, info, .. }) => {
                debug!(peer_id = %peer_id, agent = %info.agent_version, "identify received");

                // Add the observed address reported by the remote before attempting
                // rendezvous registration so the server sees an externally reachable address.
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
            }
            E::Identify(identify::Event::Sent { peer_id, .. }) => {
                trace!(peer_id = %peer_id, "identify: sent");
            }
            E::Identify(identify::Event::Pushed { peer_id, .. }) => {
                trace!(peer_id = %peer_id, "identify: pushed");
            }
            E::Identify(identify::Event::Error { peer_id, error, .. }) => {
                warn!(peer_id = %peer_id, error = %error, "identify error");
            }

            // ── Relay, DCUtR, AutoNAT, Ping ────────────────────────────
            E::Relay(relay::client::Event::ReservationReqAccepted { relay_peer_id, renewal, .. }) => {
                info!(relay_peer_id = %relay_peer_id, renewed = renewal, "relay reservation accepted");
            }
            E::Relay(relay::client::Event::OutboundCircuitEstablished { relay_peer_id, .. }) => {
                info!(relay_peer_id = %relay_peer_id, "outbound relay circuit established");
            }
            E::Relay(relay::client::Event::InboundCircuitEstablished { src_peer_id, .. }) => {
                info!(src_peer_id = %src_peer_id, "inbound relay circuit established");
            }
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

    // ── Worker Registration ───────────────────────────────────────────────

    /// Spawn an async task to verify JWT and upsert worker in DB, then send result back.
    fn spawn_register_worker_task(
        &self,
        peer_id: PeerId,
        req: decentgpu_proto::RegisterWorker,
        channel: request_response::ResponseChannel<job_codec::JobResponse>,
    ) {
        let jwt_secret = self.jwt_secret.clone();
        let db = self.db.clone();
        let result_tx = self.reg_result_tx.clone();
        let capabilities = req.capabilities.clone();

        tokio::spawn(async move {
            let (ack, saved_caps) = match verify_and_register_worker(&req.jwt_token, peer_id, capabilities, jwt_secret.as_deref(), db.as_ref()).await {
                Ok((user_id, caps)) => {
                    info!(%peer_id, %user_id, "worker registered and linked to user account");
                    (RegisterWorkerAck {
                        success: true,
                        error:   String::new(),
                        user_id: user_id.to_string(),
                    }, Some(caps))
                }
                Err(e) => {
                    warn!(%peer_id, error = %e, "worker registration failed");
                    (RegisterWorkerAck {
                        success: false,
                        error:   e.to_string(),
                        user_id: String::new(),
                    }, None)
                }
            };
            let _ = result_tx.send(RegistrationResult {
                peer_id,
                ack,
                capabilities: saved_caps,
                response_channel: channel,
            }).await;
        });
    }

    // ── Gossipsub message decoding ────────────────────────────────────────

    fn handle_gossip_message(&mut self, source: PeerId, message: gossipsub::Message) {
        let envelope = match Envelope::decode(message.data.as_ref()) {
            Ok(e) => e,
            Err(e) => {
                warn!(source = %source, error = %e, "failed to decode gossipsub envelope");
                return;
            }
        };

        match envelope.payload {
            Some(Payload::HeartBeat(HeartBeat {
                peer_id: pid_str,
                uptime_percent,
                is_busy,
                jobs_completed,
            })) => {
                let peer_id = message.source.unwrap_or(source);
                info!(
                    peer_id = %peer_id, is_busy, uptime_percent, jobs_completed,
                    "worker heartbeat received"
                );
                let _ = self.worker_tx.try_send(WorkerEvent::Heartbeat {
                    peer_id,
                    uptime: uptime_percent,
                    is_busy,
                    jobs_completed,
                });
                let _ = pid_str;
            }
            other => {
                warn!(source = %source, payload = ?other, "unexpected gossipsub message type");
            }
        }
    }
}

/// Verify a JWT token and upsert a worker record in the database.
///
/// Returns `(user_id, capabilities)` on success so the caller can emit
/// a `WorkerEvent::Online` with the correct capability set.
async fn verify_and_register_worker(
    jwt_token:    &str,
    peer_id:      PeerId,
    capabilities: Option<decentgpu_proto::WorkerCapabilities>,
    jwt_secret:   Option<&[u8]>,
    db:           Option<&crate::db::Database>,
) -> anyhow::Result<(Uuid, decentgpu_proto::WorkerCapabilities)> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let jwt_secret = jwt_secret.context("jwt_secret not configured")?;
    let db = db.context("database not configured")?;

    // Decode and verify the JWT.
    let token_data = decode::<serde_json::Value>(
        jwt_token,
        &DecodingKey::from_secret(jwt_secret),
        &Validation::default(),
    )
    .context("invalid JWT token")?;

    let sub = token_data.claims
        .get("sub")
        .and_then(|v| v.as_str())
        .context("missing 'sub' claim in JWT")?;

    let user_id: Uuid = sub.parse().context("invalid user_id in JWT sub claim")?;

    // Serialize capabilities to JSON for DB storage.
    let caps = capabilities.unwrap_or_default();
    let capabilities_json = serialize_capabilities(Some(caps.clone()));
    info!(%peer_id, capabilities = %capabilities_json, "upserting worker with capabilities");

    // Upsert worker record.
    db.workers.upsert(&peer_id.to_string(), user_id, &capabilities_json)
        .await
        .map_err(|e| anyhow::anyhow!("db upsert error: {e}"))?;

    Ok((user_id, caps))
}

/// Convert proto WorkerCapabilities to a JSON value for database storage.
fn serialize_capabilities(caps: Option<decentgpu_proto::WorkerCapabilities>) -> serde_json::Value {
    let Some(caps) = caps else {
        return serde_json::json!({ "gpus": [], "ram_mb": 0, "disk_mb": 0, "os": "", "worker_version": "" });
    };

    let gpus: Vec<serde_json::Value> = caps.gpus.iter().map(|g| {
        let backend_str = match decentgpu_proto::GpuBackend::try_from(g.backend).unwrap_or_default() {
            decentgpu_proto::GpuBackend::Cuda    => "cuda",
            decentgpu_proto::GpuBackend::Metal   => "metal",
            decentgpu_proto::GpuBackend::Rocm    => "rocm",
            decentgpu_proto::GpuBackend::CpuOnly => "cpu_only",
        };
        serde_json::json!({
            "name":    g.name,
            "vram_mb": g.vram_mb,
            "backend": backend_str,
        })
    }).collect();

    let cpu = caps.cpu.as_ref().map(|c| serde_json::json!({
        "model":    c.model,
        "cores":    c.cores,
        "threads":  c.threads,
        "freq_mhz": c.freq_mhz,
    })).unwrap_or(serde_json::Value::Null);

    serde_json::json!({
        "gpus":           gpus,
        "cpu":            cpu,
        "ram_mb":         caps.ram_mb,
        "disk_mb":        caps.disk_mb,
        "os":             caps.os,
        "worker_version": caps.worker_version,
    })
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
