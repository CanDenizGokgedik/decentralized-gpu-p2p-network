//! Worker binary download endpoints.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use super::{auth::{ApiError, AuthUser, Claims}, AppState};

/// All platforms for which a worker binary may be distributed.
const PLATFORMS: &[&str] = &[
    "linux-x86_64",
    "macos-aarch64",
    "macos-x86_64",
    "windows-x86_64",
];

/// Returns the binary filename for the given platform.
fn binary_filename(platform: &str) -> &'static str {
    if platform.starts_with("windows") {
        "decentgpu-worker.exe"
    } else {
        "decentgpu-worker"
    }
}

/// Returns the download filename (what the browser saves as).
fn download_filename(platform: &str) -> String {
    if platform.starts_with("windows") {
        "decentgpu-worker.exe".to_string()
    } else {
        format!("decentgpu-worker-{platform}")
    }
}

/// Constructs the path to a binary: `{binaries_dir}/{platform}/{binary_filename}`.
fn binary_path(binaries_dir: &str, platform: &str) -> PathBuf {
    PathBuf::from(binaries_dir)
        .join(platform)
        .join(binary_filename(platform))
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/downloads/info` — map of platform → availability info.
///
/// Does not require authentication.
pub async fn download_info(State(state): State<AppState>) -> impl IntoResponse {
    let dir = state.binaries_dir.as_ref();
    let mut platforms: HashMap<&str, serde_json::Value> = HashMap::new();

    for &platform in PLATFORMS {
        let path = binary_path(dir, platform);
        match tokio::fs::metadata(&path).await {
            Ok(meta) => {
                let size_mb = (meta.len() as f64) / 1_048_576.0;
                platforms.insert(
                    platform,
                    serde_json::json!({
                        "available":  true,
                        "size_bytes": meta.len(),
                        "size_mb":    size_mb,
                        "download_url": format!("/api/v1/downloads/worker/{platform}"),
                    }),
                );
            }
            Err(_) => {
                platforms.insert(
                    platform,
                    serde_json::json!({
                        "available": false,
                        "size_mb":   null,
                        "download_url": format!("/api/v1/downloads/worker/{platform}"),
                    }),
                );
            }
        }
    }

    Json(serde_json::json!({
        "version":  env!("CARGO_PKG_VERSION"),
        "platforms": platforms,
        "instructions": {
            "linux":   "chmod +x decentgpu-worker && WORKER_BOOTSTRAP_ADDR=/ip4/<IP>/tcp/9000 WORKER_BOOTSTRAP_PEER_ID=<PEER_ID> ./decentgpu-worker",
            "macos":   "chmod +x decentgpu-worker && WORKER_BOOTSTRAP_ADDR=/ip4/<IP>/tcp/9000 WORKER_BOOTSTRAP_PEER_ID=<PEER_ID> ./decentgpu-worker",
            "windows": "$env:WORKER_BOOTSTRAP_ADDR='/ip4/<IP>/tcp/9000'; $env:WORKER_BOOTSTRAP_PEER_ID='<PEER_ID>'; .\\decentgpu-worker.exe"
        }
    }))
}

/// `GET /api/v1/downloads/worker/:platform` — stream the worker binary.
///
/// Does not require authentication so operators can script automated downloads.
pub async fn download_worker(
    State(state): State<AppState>,
    Path(platform): Path<String>,
) -> Result<Response, ApiError> {
    if !PLATFORMS.contains(&platform.as_str()) {
        return Err(ApiError::not_found("unsupported platform"));
    }

    let dir  = state.binaries_dir.as_ref();
    let path = binary_path(dir, &platform);
    let dl_name = download_filename(&platform);

    let file = File::open(&path)
        .await
        .map_err(|_| ApiError::not_found(format!("binary for '{platform}' is not yet available")))?;

    let size = tokio::fs::metadata(&path)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .len();

    let stream = ReaderStream::new(file);
    let body   = Body::from_stream(stream);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE,        "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{dl_name}\"")),
            (header::CONTENT_LENGTH,      size.to_string()),
        ],
        body,
    )
        .into_response())
}

/// `GET /api/v1/downloads/setup-script/:platform` — generate a personalised setup script.
///
/// Requires authentication. The script embeds the user's auth token and bootstrap info.
pub async fn setup_script_handler(
    Path(platform): Path<String>,
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Response {
    let token = generate_worker_token(&auth_user, &state);

    let bootstrap_addr = std::env::var("MASTER_BOOTSTRAP_ADDR")
        .ok()
        .or_else(|| std::env::var("MASTER__BOOTSTRAP_ADDR").ok())
        .unwrap_or_else(|| state.config.bootstrap_addr.clone());

    let public_ip = std::env::var("MASTER_PUBLIC_IP")
        .ok()
        .or_else(|| std::env::var("MASTER__PUBLIC_IP").ok())
        .unwrap_or_else(|| state.config.public_ip.clone());

    let bootstrap_peer_id = std::env::var("BOOTSTRAP_PEER_ID")
        .unwrap_or_else(|_| "BOOTSTRAP_PEER_ID_NOT_CONFIGURED".to_string());

    // Detect production WSS mode: bootstrap addr uses /dns4/, /wss or custom domain.
    // In WSS mode workers must connect via WebSocket tunnel, not raw TCP.
    let is_wss = bootstrap_addr.contains("/wss") || 
                 bootstrap_addr.contains("/dns4/") || 
                 bootstrap_addr.contains(".me");

    let master_p2p_addr = if is_wss {
        // Production: master is reachable via Cloudflare-tunnelled WebSocket.
        // Allow explicit override, otherwise derive from public_ip.
        std::env::var("WORKER_MASTER_ADDR")
            .unwrap_or_else(|_| format!("/dns4/ws-master.{}/tcp/443/wss", public_ip))
    } else {
        // Local / TCP mode: extract port from listen addr and use public_ip.
        let p2p_port = state.p2p_tcp_addr
            .split('/')
            .last()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(9010);
        format!("/ip4/{}/tcp/{}", state.public_ip, p2p_port)
    };

    let (script, filename, content_type) = match platform.as_str() {
        "macos-aarch64" | "macos-x86_64" => {
            let binary_name = if platform == "macos-aarch64" {
                "decentgpu-worker-macos-aarch64"
            } else {
                "decentgpu-worker-macos-x86_64"
            };
            let script = format!(
r#"#!/bin/bash
set -e
GREEN='\033[0;32m'; BLUE='\033[0;34m'; RED='\033[0;31m'; NC='\033[0m'
BINARY="{binary_name}"
SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
echo ""
echo -e "${{BLUE}}DecentGPU Worker Kurulum${{NC}}"
echo "========================="
echo ""
if [ ! -f "$SCRIPT_DIR/$BINARY" ]; then
    echo -e "${{RED}}HATA: $BINARY bulunamadi! Programi ayni klasore indirin.${{NC}}"
    exit 1
fi
echo -e "${{GREEN}}Worker programi bulundu${{NC}}"
xattr -dr com.apple.quarantine "$SCRIPT_DIR/$BINARY" 2>/dev/null || true
chmod +x "$SCRIPT_DIR/$BINARY"
echo -e "${{GREEN}}Izinler ayarlandi${{NC}}"
mkdir -p "$HOME/.decentgpu/workspace"
echo -e "${{GREEN}}Calisma dizini hazir${{NC}}"
echo ""
echo -e "${{BLUE}}Sunucuya baglaniliyor...${{NC}}"
echo ""
export WORKER_AUTH_TOKEN="{token}"
export WORKER_BOOTSTRAP_ADDR="{bootstrap_addr}"
export WORKER_BOOTSTRAP_PEER_ID="{bootstrap_peer_id}"
export WORKER_MASTER_ADDR="{master_p2p_addr}"
export WORKER_KEYPAIR_PATH="$HOME/.decentgpu/worker.keypair"
export WORKER_WORKSPACE_PATH="$HOME/.decentgpu/workspace"
export WORKER_P2P_PORT=9030
export RUST_LOG=decentgpu_worker=info
echo -e "${{GREEN}}Worker baslatiliyor! Durdurmak icin: Ctrl+C${{NC}}"
echo "========================="
echo ""
"$SCRIPT_DIR/$BINARY"
"#,
                binary_name = binary_name,
                token = token,
                bootstrap_addr = bootstrap_addr,
                bootstrap_peer_id = bootstrap_peer_id,
                master_p2p_addr = master_p2p_addr,
            );
            (script, "decentgpu-setup.sh", "text/x-shellscript")
        }
        "linux-x86_64" => {
            let script = format!(
r#"#!/bin/bash
set -e
BINARY="decentgpu-worker-linux-x86_64"
SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
echo "DecentGPU Worker Kurulum"
echo "========================"
if [ ! -f "$SCRIPT_DIR/$BINARY" ]; then
    echo "HATA: $BINARY bulunamadi!"
    exit 1
fi
chmod +x "$SCRIPT_DIR/$BINARY"
mkdir -p "$HOME/.decentgpu/workspace"
export WORKER_AUTH_TOKEN="{token}"
export WORKER_BOOTSTRAP_ADDR="{bootstrap_addr}"
export WORKER_BOOTSTRAP_PEER_ID="{bootstrap_peer_id}"
export WORKER_MASTER_ADDR="{master_p2p_addr}"
export WORKER_KEYPAIR_PATH="$HOME/.decentgpu/worker.keypair"
export WORKER_WORKSPACE_PATH="$HOME/.decentgpu/workspace"
export WORKER_P2P_PORT=9030
export RUST_LOG=decentgpu_worker=info
echo "Hazir - Worker baslatiliyor!"
echo ""
"$SCRIPT_DIR/$BINARY"
"#,
                token = token,
                bootstrap_addr = bootstrap_addr,
                bootstrap_peer_id = bootstrap_peer_id,
                master_p2p_addr = master_p2p_addr,
            );
            (script, "decentgpu-setup.sh", "text/x-shellscript")
        }
        "windows-x86_64" => {
            let script = format!(
r#"@echo off
echo.
echo  DecentGPU Worker Kurulum
echo  ========================
set BINARY=decentgpu-worker-windows-x86_64.exe
if not exist "%~dp0%BINARY%" (
    echo  HATA: %BINARY% bulunamadi!
    pause & exit /b 1
)
echo  Worker programi bulundu.
mkdir "%USERPROFILE%\.decentgpu\workspace" 2>nul
set WORKER_AUTH_TOKEN={token}
set WORKER_BOOTSTRAP_ADDR={bootstrap_addr}
set WORKER_BOOTSTRAP_PEER_ID={bootstrap_peer_id}
set WORKER_MASTER_ADDR={master_p2p_addr}
set WORKER_KEYPAIR_PATH=%USERPROFILE%\.decentgpu\worker.keypair
set WORKER_WORKSPACE_PATH=%USERPROFILE%\.decentgpu\workspace
set WORKER_P2P_PORT=9030
set RUST_LOG=decentgpu_worker=info
echo  Hazir!
"%~dp0%BINARY%"
pause
"#,
                token = token,
                bootstrap_addr = bootstrap_addr,
                bootstrap_peer_id = bootstrap_peer_id,
                master_p2p_addr = master_p2p_addr,
            );
            (script, "decentgpu-setup.bat", "text/plain")
        }
        _ => {
            return (StatusCode::BAD_REQUEST, "Unknown platform").into_response();
        }
    };

    (
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        script,
    )
        .into_response()
}

fn generate_worker_token(auth_user: &AuthUser, state: &AppState) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(7))
        .unwrap_or_else(chrono::Utc::now)
        .timestamp() as usize;

    let claims = Claims {
        sub:   auth_user.user_id.to_string(),
        email: auth_user.email.clone(),
        role:  auth_user.role.clone(),
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&state.jwt_secret),
    )
    .unwrap_or_default()
}
