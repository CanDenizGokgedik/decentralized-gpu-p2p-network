//! Axum router — wires together all API routes.

use axum::{
    extract::State,
    http::{HeaderValue, Method, header},
    middleware,
    routing::{get, patch, post},
    Json, Router,
    extract::DefaultBodyLimit,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use super::{admin, auth, credits, downloads, jobs, logs, terminal, workers, AppState};

/// Build the complete Axum router with all routes attached.
pub fn build(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
            "http://localhost:3001".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
            Method::HEAD,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ORIGIN,
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            header::ACCESS_CONTROL_REQUEST_METHOD,
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600));

    // ── Public routes (no auth) ───────────────────────────────────────────────
    let public = Router::new()
        .route("/api/auth/register",              post(auth::register))
        .route("/api/auth/login",                 post(auth::login))
        .route("/api/compute-units/pricing",      get(credits::get_pricing))
        .route("/api/downloads/info",             get(downloads::download_info))
        .route("/api/downloads/worker/:platform", get(downloads::download_worker))
        .route("/health",                            get(health));

    // ── Authenticated routes ──────────────────────────────────────────────────
    let protected = Router::new()
        // Auth
        .route("/api/auth/me", get(auth::me))
        // Jobs
        .route("/api/jobs",              get(jobs::list_jobs).post(jobs::create_job))
        .route("/api/jobs/:id",          get(jobs::get_job))
        .route("/api/jobs/:id/upload",   post(jobs::upload_job_file))
        .route("/api/jobs/:id/cancel",   post(jobs::cancel_job))
        .route("/api/jobs/:id/result",   get(jobs::download_result))
        .route("/api/jobs/:id/logs",     get(logs::stream_logs))
        .route("/api/jobs/:id/terminal", get(terminal::ws_terminal))
        // Workers
        .route("/api/workers",           get(workers::list_workers))
        .route("/api/workers/me",        get(workers::get_my_worker))
        .route("/api/workers/:peer_id",  get(workers::get_worker_by_peer_id))
        // Downloads (authenticated)
        .route("/api/downloads/setup-script/:platform", get(downloads::setup_script_handler))
        // Compute Units
        .route("/api/compute-units/balance",      get(credits::get_balance))
        .route("/api/compute-units/transactions", get(credits::list_transactions))
        .route("/api/compute-units/allocate",     post(credits::allocate))
        // Admin
        .route("/api/admin/users",                       get(admin::list_users))
        .route("/api/admin/users/:id",                   get(admin::get_user))
        .route("/api/admin/users/:id/role",              patch(admin::update_user_role))
        .route("/api/admin/stats",                       get(admin::get_stats))
        .route("/api/admin/workers/:peer_id/disconnect", post(admin::disconnect_worker))
        .route("/api/admin/downloads/worker/:platform",  post(admin::upload_worker_binary))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::jwt_middleware,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(DefaultBodyLimit::disable())
        .with_state(state)
}

/// `GET /health` — quick connectivity check.
async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.db.pool)
        .await
        .is_ok();

    let workers_online = state.db.workers.count_online().await.unwrap_or(0);

    Json(serde_json::json!({
        "status":         if db_ok { "ok" } else { "degraded" },
        "service":        "master",
        "db":             if db_ok { "connected" } else { "error" },
        "workers_online": workers_online,
    }))
}
