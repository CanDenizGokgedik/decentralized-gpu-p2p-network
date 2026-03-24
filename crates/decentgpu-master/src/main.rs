//! DecentGPU Master Node — central coordinator and API server.

use decentgpu_master::{api, db, p2p, scheduler};

use anyhow::Result;
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{routing::any, Router};
use dashmap::DashMap;
use decentgpu_common::config::{load_config, MasterConfig};
use rand::rngs::OsRng;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("decentgpu_master=debug".parse()?)
                .add_directive("libp2p=info".parse()?),
        )
        .json()
        .init();

    let mut cfg: MasterConfig = load_config("MASTER")?;
    info!("master node starting");

    // Override database_url from DATABASE_URL env var if not set via MASTER__ prefix
    if cfg.database_url.is_empty() {
        if let Ok(db_url) = std::env::var("DATABASE_URL")
            .or_else(|_| std::env::var("MASTER_DATABASE_URL"))
        {
            info!(url = %db_url, "database_url loaded from DATABASE_URL env var");
            cfg.database_url = db_url;
        }
    }
    info!(database_url_set = !cfg.database_url.is_empty(), "database_url status");

    // ── Database (optional) ────────────────────────────────────────────────
    let maybe_pool = if cfg.database_url.is_empty() {
        info!("no database_url configured — API disabled");
        None
    } else {
        let pool = db::connect(&cfg.database_url).await?;
        db::run_migrations(&pool).await?;

        let database = db::Database::new(pool.clone());
        let n = database
            .workers
            .mark_all_offline()
            .await
            .map_err(|e| anyhow::anyhow!("mark_all_offline: {e}"))?;
        info!(count = n, "marked stale workers offline");

        // Bootstrap admin user from environment variables (idempotent).
        if let (Ok(email), Ok(password)) = (
            std::env::var("ADMIN_EMAIL"),
            std::env::var("ADMIN_PASSWORD"),
        ) {
            bootstrap_admin(&database, &pool, &email, &password).await;
        }

        Some(pool)
    };

    // ── P2P layer ─────────────────────────────────────────────────────────
    // Pass jwt_secret and db to P2P layer so it can handle RegisterWorker messages.
    let p2p_jwt_secret = Some(cfg.jwt_secret.as_bytes().to_vec());
    let p2p_db = maybe_pool.as_ref().map(|pool| db::Database::new(pool.clone()));

    let log_bus: Arc<DashMap<Uuid, broadcast::Sender<serde_json::Value>>> = Arc::new(DashMap::new());
    let p2p_log_bus = Arc::clone(&log_bus);

    let (p2p_handle, job_tx, worker_rx) =
        p2p::start_with_auth(cfg.clone(), p2p_jwt_secret, p2p_db, Some(p2p_log_bus)).await?;

    // ── HTTP API + Scheduler (only when DB is configured) ─────────────────
    if let Some(pool) = maybe_pool {
        let db = Arc::new(db::Database::new(pool.clone()));

        // Fan-out worker_rx to both the scheduler and the DB updater.
        let (sched_tx, sched_rx) = tokio::sync::mpsc::channel::<p2p::WorkerEvent>(256);
        let (db_tx, db_rx)       = tokio::sync::mpsc::channel::<p2p::WorkerEvent>(256);

        tokio::spawn({
            let mut rx = worker_rx;
            async move {
                while let Some(ev) = rx.recv().await {
                    let ev2 = clone_worker_event(&ev);
                    let _ = sched_tx.send(ev).await;
                    let _ = db_tx.send(ev2).await;
                }
            }
        });

        // DB updater: keep worker online/offline status in sync.
        let db2 = Arc::clone(&db);
        tokio::spawn(async move {
            let mut rx = db_rx;
            while let Some(event) = rx.recv().await {
                match event {
                    p2p::WorkerEvent::Heartbeat { peer_id, .. }
                    | p2p::WorkerEvent::Online { peer_id, .. } => {
                        if let Err(e) = db2.workers.set_online(&peer_id.to_string(), true).await {
                            tracing::warn!(error = %e, "failed to mark worker online");
                        }
                    }
                    p2p::WorkerEvent::Offline { peer_id } => {
                        if let Err(e) = db2.workers.set_online(&peer_id.to_string(), false).await {
                            tracing::warn!(error = %e, "failed to mark worker offline");
                        }
                    }
                }
            }
        });

        // Scheduler.
        let storage_path = Arc::new(cfg.storage_path.clone());
        let (sched, scheduler_handle) = scheduler::JobScheduler::new(
            Arc::clone(&db),
            job_tx.clone(), // clone — the API layer also needs a sender
            sched_rx,
            Arc::clone(&storage_path),
        );
        tokio::spawn(sched.run());

        let state = api::AppState::new(cfg.clone(), pool, scheduler_handle, job_tx, log_bus).await?;
        api::serve(cfg, state).await?;
    } else {
        // No DB: drain events to avoid backpressure; serve 503 for all routes.
        tokio::spawn(async move {
            let mut rx = worker_rx;
            while rx.recv().await.is_some() {}
        });

        let api_addr: std::net::SocketAddr = cfg.api_addr.parse()?;
        info!(addr = %api_addr, "starting stub API (503 — no database)");
        let app = Router::new().fallback(any(|| async {
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "database not configured",
            )
        }));
        let listener = tokio::net::TcpListener::bind(api_addr).await?;
        axum::serve(listener, app).await?;
    }

    p2p_handle.await??;
    Ok(())
}

// ── Admin bootstrap ────────────────────────────────────────────────────────────

/// Create an admin user from `ADMIN_EMAIL` + `ADMIN_PASSWORD` env vars if they
/// do not already exist. Runs at every startup — safe to call multiple times.
async fn bootstrap_admin(
    database: &db::Database,
    pool:     &sqlx::PgPool,
    email:    &str,
    password: &str,
) {
    if database.users.find_by_email(email).await.ok().flatten().is_some() {
        info!(%email, "admin user already exists — skipping bootstrap");
        return;
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = match Argon2::default().hash_password(password.as_bytes(), &salt) {
        Ok(h)  => h.to_string(),
        Err(e) => {
            tracing::error!(error = %e, "failed to hash admin password");
            return;
        }
    };

    match database.users.create(email, &hash, "admin").await {
        Ok(user) => {
            // Initialize CU balance for the admin account.
            let _ = sqlx::query!(
                "INSERT INTO compute_unit_balances (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
                user.id,
            )
            .execute(pool)
            .await;

            info!(%email, user_id = %user.id, "admin user bootstrapped");
        }
        Err(e) => {
            tracing::warn!(error = %e, "admin bootstrap failed");
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn clone_worker_event(ev: &p2p::WorkerEvent) -> p2p::WorkerEvent {
    match ev {
        p2p::WorkerEvent::Online { peer_id, capabilities } => p2p::WorkerEvent::Online {
            peer_id:      *peer_id,
            capabilities: capabilities.clone(),
        },
        p2p::WorkerEvent::Offline { peer_id } => {
            p2p::WorkerEvent::Offline { peer_id: *peer_id }
        }
        p2p::WorkerEvent::Heartbeat { peer_id, uptime, is_busy, jobs_completed } => {
            p2p::WorkerEvent::Heartbeat {
                peer_id:        *peer_id,
                uptime:         *uptime,
                is_busy:        *is_busy,
                jobs_completed: *jobs_completed,
            }
        }
    }
}
