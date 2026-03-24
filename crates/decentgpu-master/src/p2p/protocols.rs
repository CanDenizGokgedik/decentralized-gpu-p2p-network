//! Protocol identifiers and namespace/topic constants for DecentGPU P2P.

/// Protocol for job assignment: master → worker.
pub const PROTO_JOB: &str = "/decentgpu/job/1.0.0";

/// Protocol for log streaming: worker → master.
pub const PROTO_STREAM: &str = "/decentgpu/stream/1.0.0";

/// Protocol for binary file transfer (Docker image or result tar).
pub const PROTO_TRANSFER: &str = "/decentgpu/transfer/1.0.0";

/// Rendezvous namespace used by the master node.
pub const NAMESPACE_MASTER: &str = "master";

/// Rendezvous namespace used by worker nodes.
pub const NAMESPACE_WORKERS: &str = "workers";

/// Gossipsub topic for worker heartbeats.
pub const TOPIC_HEARTBEAT: &str = "/decentgpu/workers/heartbeat";

/// Gossipsub topic for job status broadcasts.
pub const TOPIC_JOB_STATUS: &str = "/decentgpu/jobs/status";
