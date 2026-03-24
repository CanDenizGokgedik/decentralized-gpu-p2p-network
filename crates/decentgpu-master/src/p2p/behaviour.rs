//! Master node combined [`NetworkBehaviour`].

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};

use libp2p::{
    autonat, dcutr, gossipsub, identify, identity::Keypair,
    kad::{self, store::MemoryStore},
    ping, relay, rendezvous,
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    StreamProtocol,
};

use super::protocols;
pub use job_codec::{JobCodec, JobRequest, JobResponse};

/// All P2P behaviours for the master node.
#[derive(NetworkBehaviour)]
pub struct MasterBehaviour {
    pub kademlia:  kad::Behaviour<MemoryStore>,
    pub rendezvous: rendezvous::client::Behaviour,
    pub relay:     relay::client::Behaviour,
    pub dcutr:     dcutr::Behaviour,
    pub autonat:   autonat::Behaviour,
    pub identify:  identify::Behaviour,
    pub ping:      ping::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    /// Request-response: job assignment (master → worker).
    pub job_rr:    request_response::Behaviour<JobCodec>,
}

impl MasterBehaviour {
    pub fn new(
        local_peer_id: libp2p::PeerId,
        key: &Keypair,
        relay_client: relay::client::Behaviour,
    ) -> Self {
        let kad_config = kad::Config::new(StreamProtocol::new("/decentgpu/kad/1.0.0"));
        let mut kademlia = kad::Behaviour::with_config(
            local_peer_id,
            MemoryStore::new(local_peer_id),
            kad_config,
        );
        kademlia.set_mode(Some(kad::Mode::Server));

        let message_id_fn = |message: &gossipsub::Message| {
            let mut hasher = DefaultHasher::new();
            if let Some(ref src) = message.source { src.hash(&mut hasher); }
            message.sequence_number.hash(&mut hasher);
            gossipsub::MessageId::from(hasher.finish().to_be_bytes().to_vec())
        };
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .mesh_n(6).mesh_n_low(4).mesh_n_high(12)
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .expect("gossipsub config");

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(key.clone()),
            gossipsub_config,
        ).expect("gossipsub init");
        gossipsub.subscribe(&gossipsub::IdentTopic::new(protocols::TOPIC_HEARTBEAT)).expect("sub hb");
        gossipsub.subscribe(&gossipsub::IdentTopic::new(protocols::TOPIC_JOB_STATUS)).expect("sub js");

        let job_rr = request_response::Behaviour::new(
            vec![(StreamProtocol::new(protocols::PROTO_JOB), ProtocolSupport::Full)],
            request_response::Config::default().with_request_timeout(Duration::from_secs(30)),
        );

        Self {
            kademlia,
            rendezvous: rendezvous::client::Behaviour::new(key.clone()),
            relay: relay_client,
            dcutr: dcutr::Behaviour::new(local_peer_id),
            autonat: autonat::Behaviour::new(local_peer_id, autonat::Config::default()),
            identify: identify::Behaviour::new(
                identify::Config::new("/decentgpu/identify/1.0.0".into(), key.public())
                    .with_agent_version("master/0.1.0".into()),
            ),
            ping: ping::Behaviour::new(ping::Config::new()),
            gossipsub,
            job_rr,
        }
    }
}

// ── Job request-response codec ────────────────────────────────────────────────

pub mod job_codec {
    use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use libp2p::request_response;
    use prost::Message as _;

    #[derive(Debug, Clone)]
    pub struct JobRequest(pub Vec<u8>);

    #[derive(Debug, Clone)]
    pub struct JobResponse(pub Vec<u8>);

    #[derive(Debug, Clone, Default)]
    pub struct JobCodec;

    #[async_trait::async_trait]
    impl request_response::Codec for JobCodec {
        type Protocol = libp2p::StreamProtocol;
        type Request  = JobRequest;
        type Response = JobResponse;

        async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T)
            -> std::io::Result<Self::Request>
        where T: AsyncRead + Unpin + Send {
            let len = read_u32(io).await?;
            let mut buf = vec![0u8; len as usize];
            io.read_exact(&mut buf).await?;
            Ok(JobRequest(buf))
        }

        async fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T)
            -> std::io::Result<Self::Response>
        where T: AsyncRead + Unpin + Send {
            let len = read_u32(io).await?;
            let mut buf = vec![0u8; len as usize];
            io.read_exact(&mut buf).await?;
            Ok(JobResponse(buf))
        }

        async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Self::Request)
            -> std::io::Result<()>
        where T: AsyncWrite + Unpin + Send {
            io.write_all(&(req.0.len() as u32).to_be_bytes()).await?;
            io.write_all(&req.0).await?;
            io.flush().await
        }

        async fn write_response<T>(&mut self, _: &Self::Protocol, io: &mut T, resp: Self::Response)
            -> std::io::Result<()>
        where T: AsyncWrite + Unpin + Send {
            io.write_all(&(resp.0.len() as u32).to_be_bytes()).await?;
            io.write_all(&resp.0).await?;
            io.flush().await
        }
    }

    async fn read_u32<T: AsyncRead + Unpin>(io: &mut T) -> std::io::Result<u32> {
        let mut buf = [0u8; 4];
        io.read_exact(&mut buf).await?;
        Ok(u32::from_be_bytes(buf))
    }

    pub fn encode<M: prost::Message>(msg: &M) -> Vec<u8> { msg.encode_to_vec() }
}
