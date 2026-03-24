//! Database connection pool, migrations, and repository aggregation.

pub mod compute_units;
pub mod jobs;
pub mod users;
pub mod workers;

pub use compute_units::ComputeUnitRepository;
pub use jobs::JobRepository;
pub use users::UserRepository;
pub use workers::WorkerRepository;

use anyhow::{Context, Result};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{sync::Arc, time::Duration};

/// Create a connection pool with production-grade settings.
pub async fn connect(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")
}

/// Run all pending SQLx migrations.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("src/db/migrations")
        .run(pool)
        .await
        .context("failed to run database migrations")
}

/// All repositories bundled together — share via `Arc<Database>`.
#[derive(Clone)]
pub struct Database {
    pub pool:          PgPool,
    pub users:         UserRepository,
    pub jobs:          JobRepository,
    pub workers:       WorkerRepository,
    pub compute_units: ComputeUnitRepository,
}

impl Database {
    pub fn new(pool: PgPool) -> Self {
        Self {
            users:         UserRepository::new(pool.clone()),
            jobs:          JobRepository::new(pool.clone()),
            workers:       WorkerRepository::new(pool.clone()),
            compute_units: ComputeUnitRepository::new(pool.clone()),
            pool,
        }
    }

    pub fn arc(pool: PgPool) -> Arc<Self> {
        Arc::new(Self::new(pool))
    }
}
