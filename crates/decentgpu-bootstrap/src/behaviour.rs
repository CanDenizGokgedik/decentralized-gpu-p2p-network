//! Combined [`NetworkBehaviour`] for the bootstrap/rendezvous/relay node.

use libp2p::{identify, ping, relay, rendezvous, swarm::NetworkBehaviour};

/// All P2P behaviours combined for the bootstrap node.
#[derive(NetworkBehaviour)]
pub struct BootstrapBehaviour {
    /// Identify: exchange peer info on connection.
    pub identify: identify::Behaviour,
    /// Ping: measure round-trip latency.
    pub ping: ping::Behaviour,
    /// Rendezvous server: allow peers to register and discover each other.
    pub rendezvous: rendezvous::server::Behaviour,
    /// Relay v2 server: relay connections for NAT-traversal.
    pub relay: relay::Behaviour,
}
