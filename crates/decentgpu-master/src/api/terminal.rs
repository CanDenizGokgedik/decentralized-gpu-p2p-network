//! WebSocket terminal handler for live job log streaming with stored-log replay.

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::time::{interval, Duration};
use uuid::Uuid;

use super::{auth::ApiError, auth::AuthUser, AppState};

/// `GET /api/jobs/:id/terminal` — upgrade to WebSocket for job terminal.
pub async fn ws_terminal(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    let job = state
        .db
        .jobs
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("job not found"))?;
    if job.hirer_id != user.user_id {
        return Err(ApiError::forbidden("not your job"));
    }

    Ok(ws.on_upgrade(move |socket| handle_terminal_socket(socket, state, id)))
}

/// Drive a single WebSocket connection for a job terminal.
///
/// Protocol (server → client):
/// - `{"type":"connected","job_id":"..."}` — handshake
/// - `{"type":"log","ts":ms,"level":"INFO","data":"line\r\n"}` — replayed / live log line
/// - `{"type":"replay_complete","count":N}` — all stored logs sent
/// - `{"type":"job_done","status":"completed"|"failed"|...}` — job finished, socket closes
/// - `{"type":"ping"}` — keepalive, no display
async fn handle_terminal_socket(mut socket: WebSocket, state: AppState, job_id: Uuid) {
    const TERMINAL_STATUSES: &[&str] = &["completed", "failed", "cancelled"];

    // ── 1. Welcome handshake ─────────────────────────────────────────────
    let welcome = serde_json::json!({ "type": "connected", "job_id": job_id }).to_string();
    if socket.send(Message::Text(welcome)).await.is_err() {
        return;
    }

    // ── 2. Replay stored logs ────────────────────────────────────────────
    let stored_logs = match state.db.jobs.get_logs_since(job_id, 0, 10_000).await {
        Ok(logs) => logs,
        Err(e) => {
            tracing::warn!(%job_id, error = %e, "failed to read logs for replay");
            vec![]
        }
    };
    let replay_count = stored_logs.len();
    for (ts, level, message) in stored_logs {
        let msg = serde_json::json!({
            "type":  "log",
            "ts":    ts,
            "level": level,
            "data":  format!("{}\r\n", message),
        })
        .to_string();
        if socket.send(Message::Text(msg)).await.is_err() {
            return;
        }
    }
    let replay_done = serde_json::json!({ "type": "replay_complete", "count": replay_count }).to_string();
    if socket.send(Message::Text(replay_done)).await.is_err() {
        return;
    }

    // ── 3. Check if job is already in a terminal state ───────────────────
    let job_status = match state.db.jobs.find_by_id(job_id).await {
        Ok(Some(j)) => j.status,
        _ => "unknown".to_string(),
    };
    if TERMINAL_STATUSES.contains(&job_status.as_str()) {
        let done = serde_json::json!({ "type": "job_done", "status": job_status }).to_string();
        let _ = socket.send(Message::Text(done)).await;
        return;
    }

    // ── 4. Subscribe to live log bus (for jobs still running) ────────────
    let mut log_rx = state
        .log_bus
        .entry(job_id)
        .or_insert_with(|| {
            let (tx, _rx) = tokio::sync::broadcast::channel(256);
            tx
        })
        .subscribe();

    // ── 5. Event loop: forward live logs + keepalive pings ───────────────
    let mut ping_ticker = interval(Duration::from_secs(15));
    // Skip the first tick so we don't immediately ping before any work is done.
    ping_ticker.tick().await;

    let (mut ws_tx, mut ws_rx) = socket.split();

    loop {
        tokio::select! {
            // Incoming client messages (e.g. close frame)
            msg = ws_rx.next() => {
                match msg {
                    None
                    | Some(Ok(Message::Close(_)))
                    | Some(Err(_)) => break,
                    _ => {}
                }
            }

            // Live log from the broadcast bus
            log_msg = log_rx.recv() => {
                match log_msg {
                    Ok(val) => {
                        if ws_tx.send(Message::Text(val.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Receiver fell behind; keep going — missed entries are in DB.
                    }
                }
            }

            // Periodic keepalive / terminal-status check
            _ = ping_ticker.tick() => {
                // Re-check job status to detect completion while subscribed.
                let current_status = match state.db.jobs.find_by_id(job_id).await {
                    Ok(Some(j)) => j.status,
                    _ => continue,
                };
                if TERMINAL_STATUSES.contains(&current_status.as_str()) {
                    let done = serde_json::json!({ "type": "job_done", "status": current_status }).to_string();
                    let _ = ws_tx.send(Message::Text(done)).await;
                    break;
                }
                // Not done yet — send a keepalive ping.
                let ping = serde_json::json!({ "type": "ping" }).to_string();
                if ws_tx.send(Message::Text(ping)).await.is_err() {
                    break;
                }
            }
        }
    }
}
