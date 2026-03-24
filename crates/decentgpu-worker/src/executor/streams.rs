//! Utilities for forwarding stdout/stderr over P2P log streams.

use anyhow::Result;
use chrono::Utc;
use decentgpu_proto::{envelope::Payload, Envelope, LogLevel, LogLine};
use futures::io::AsyncWriteExt;
use prost::Message;
use tracing::instrument;

/// Write a [`LogLine`] to a P2P stream as a length-prefixed frame.
///
/// Frame format: 4-byte big-endian length followed by protobuf bytes.
#[instrument(skip(stream, message), fields(job_id = %job_id))]
pub async fn send_log_line(
    stream: &mut libp2p::Stream,
    job_id: &str,
    message: &str,
    is_stderr: bool,
) -> Result<()> {
    let log_line = LogLine {
        job_id: job_id.to_string(),
        timestamp_ms: Utc::now().timestamp_millis(),
        level: if is_stderr {
            LogLevel::Stderr as i32
        } else {
            LogLevel::Stdout as i32
        },
        message: message.to_string(),
    };

    let envelope = Envelope {
        payload: Some(Payload::LogLine(log_line)),
    };

    let encoded = envelope.encode_to_vec();
    let len = (encoded.len() as u32).to_be_bytes();

    stream.write_all(&len).await?;
    stream.write_all(&encoded).await?;
    stream.flush().await?;

    Ok(())
}
