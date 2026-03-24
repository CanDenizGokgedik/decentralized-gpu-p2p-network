#![deny(clippy::all)]
#![allow(clippy::derive_partial_eq_without_eq)]

//! Protobuf-generated types for DecentGPU P2P protocol messages.

// Include prost-generated code.
include!(concat!(env!("OUT_DIR"), "/decentgpu.rs"));

use prost::Message;

/// Maximum allowed size for any P2P message envelope (16 MiB).
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Maximum chunk size for file transfers (64 KiB).
pub const CHUNK_SIZE: usize = 64 * 1024;

impl Envelope {
    /// Encode this envelope to a length-prefixed byte buffer.
    pub fn encode_length_delimited_to_vec(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_len() + 4);
        self.encode_length_delimited(&mut buf)
            .expect("encoding to Vec is infallible");
        buf
    }

    /// Decode an envelope from a length-prefixed byte slice.
    pub fn decode_length_delimited_from(buf: &[u8]) -> Result<Self, prost::DecodeError> {
        Self::decode_length_delimited(buf)
    }
}

impl GpuBackend {
    /// Return a human-readable name for this backend.
    pub fn as_str(self) -> &'static str {
        match self {
            GpuBackend::CpuOnly => "cpu_only",
            GpuBackend::Cuda => "cuda",
            GpuBackend::Metal => "metal",
            GpuBackend::Rocm => "rocm",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_envelope() {
        let env = Envelope {
            payload: Some(envelope::Payload::HeartBeat(HeartBeat {
                peer_id: "12D3KooWtest".into(),
                uptime_percent: 99.5,
                is_busy: false,
                jobs_completed: 42,
            })),
        };
        let bytes = env.encode_length_delimited_to_vec();
        let decoded = Envelope::decode_length_delimited_from(&bytes).unwrap();
        assert_eq!(env, decoded);
    }
}
