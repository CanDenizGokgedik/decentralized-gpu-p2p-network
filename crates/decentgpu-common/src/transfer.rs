//! P2P wire-framing and chunked file transfer utilities.
//!
//! `write_length_prefixed` / `read_length_prefixed` frame protobuf messages
//! over any async byte stream (e.g. `libp2p::Stream`).
//!
//! `send_file_chunked` / `receive_file_chunked` transfer arbitrary files in
//! 64 KiB chunks with per-chunk SHA-256 integrity and ACK-based backpressure.

use std::path::Path;

use decentgpu_proto::{envelope::Payload, Envelope, TransferAck, TransferChunk};
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use prost::Message as _;
use sha2::{Digest, Sha256};

const CHUNK_SIZE: usize = 64 * 1024; // 64 KiB

// ── Length-prefixed frame helpers ─────────────────────────────────────────────

/// Write a protobuf message to a stream as a 4-byte big-endian length prefix
/// followed by the encoded bytes. Works with `libp2p::Stream` (futures IO).
pub async fn write_length_prefixed<S, M>(stream: &mut S, msg: &M) -> anyhow::Result<()>
where
    S: AsyncWrite + Unpin,
    M: prost::Message,
{
    let bytes = msg.encode_to_vec();
    let len = bytes.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}

/// Read one length-prefixed message from the stream.
///
/// Returns `Err` if the declared length exceeds 512 MiB.
pub async fn read_length_prefixed<S>(stream: &mut S) -> anyhow::Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 512 * 1024 * 1024 {
        anyhow::bail!("message too large: {len} bytes");
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

// ── Chunked file transfer ─────────────────────────────────────────────────────

/// Send a file over an open bidirectional stream in 64 KiB chunks.
///
/// Each chunk is SHA-256 hashed and the receiver sends a `TransferAck` before
/// the next chunk is sent (backpressure). Returns total bytes transferred.
pub async fn send_file_chunked<S>(
    stream:      &mut S,
    file_path:   &Path,
    transfer_id: &str,
) -> anyhow::Result<u64>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let data = tokio::fs::read(file_path).await?;
    let file_size = data.len() as u64;
    let mut offset = 0u64;
    let mut remaining = data.as_slice();

    while !remaining.is_empty() {
        let n = remaining.len().min(CHUNK_SIZE);
        let chunk = &remaining[..n];
        remaining = &remaining[n..];
        let is_last = remaining.is_empty();

        let mut hasher = Sha256::new();
        hasher.update(chunk);
        let sha256 = hex::encode(hasher.finalize());

        write_length_prefixed(
            stream,
            &Envelope {
                payload: Some(Payload::TransferChunk(TransferChunk {
                    transfer_id: transfer_id.to_string(),
                    offset,
                    data: chunk.to_vec(),
                    is_last,
                    sha256,
                })),
            },
        )
        .await?;

        // Wait for ACK (60 s timeout).
        let ack_buf = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            read_length_prefixed(stream),
        )
        .await
        .map_err(|_| anyhow::anyhow!("ACK timeout at offset {offset}"))??;

        let ack_env = Envelope::decode(ack_buf.as_slice())?;
        match ack_env.payload {
            Some(Payload::TransferAck(ack)) if ack.ok => {
                offset += n as u64;
            }
            Some(Payload::TransferAck(ack)) => {
                anyhow::bail!("transfer rejected at offset {}", ack.offset);
            }
            _ => anyhow::bail!("invalid ACK envelope"),
        }
    }

    tracing::info!(transfer_id, total_bytes = file_size, "file send complete");
    Ok(file_size)
}

/// Receive a chunked file over an open bidirectional stream.
///
/// Writes the reassembled file to `output_path`. Sends a per-chunk
/// `TransferAck` after verifying SHA-256 integrity. Returns bytes written.
pub async fn receive_file_chunked<S>(
    stream:      &mut S,
    output_path: &Path,
) -> anyhow::Result<u64>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file_data: Vec<u8> = Vec::new();
    let mut total = 0u64;

    loop {
        let frame = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            read_length_prefixed(stream),
        )
        .await
        .map_err(|_| anyhow::anyhow!("chunk timeout"))??;

        let env = Envelope::decode(frame.as_slice())?;
        let chunk = match env.payload {
            Some(Payload::TransferChunk(c)) => c,
            _ => anyhow::bail!("expected TransferChunk"),
        };

        let mut hasher = Sha256::new();
        hasher.update(&chunk.data);
        let computed = hex::encode(hasher.finalize());
        let ok = computed == chunk.sha256;

        write_length_prefixed(
            stream,
            &Envelope {
                payload: Some(Payload::TransferAck(TransferAck {
                    transfer_id: chunk.transfer_id.clone(),
                    offset: chunk.offset,
                    ok,
                })),
            },
        )
        .await?;

        if !ok {
            anyhow::bail!(
                "integrity check failed at offset {} (got {computed}, want {})",
                chunk.offset,
                chunk.sha256
            );
        }

        file_data.extend_from_slice(&chunk.data);
        total += chunk.data.len() as u64;

        if chunk.is_last {
            break;
        }
    }

    tokio::fs::write(output_path, &file_data).await?;
    tracing::info!(path = %output_path.display(), bytes = total, "file receive complete");
    Ok(total)
}
