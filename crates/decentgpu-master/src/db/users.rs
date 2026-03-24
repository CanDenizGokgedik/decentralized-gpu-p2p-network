//! Database queries for the `users` table.

use chrono::{DateTime, Utc};
use decentgpu_common::DecentGpuError;
use sqlx::PgPool;
use uuid::Uuid;

type Result<T> = std::result::Result<T, DecentGpuError>;

/// A row from the `users` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id:            Uuid,
    pub email:         String,
    pub password_hash: String,
    pub role:          String,
    pub created_at:    DateTime<Utc>,
}

/// Repository for user operations.
#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new user, returning the created row.
    pub async fn create(
        &self,
        email:         &str,
        password_hash: &str,
        role:          &str,
    ) -> Result<User> {
        sqlx::query_as!(
            User,
            r#"INSERT INTO users (email, password_hash, role)
               VALUES ($1, $2, $3)
               RETURNING id, email, password_hash, role, created_at"#,
            email,
            password_hash,
            role,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db)
                if db.constraint() == Some("users_email_key") =>
            {
                DecentGpuError::Conflict("email already registered".into())
            }
            other => DecentGpuError::Database(other.to_string()),
        })
    }

    /// Find a user by email address.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, role, created_at
               FROM users WHERE email = $1"#,
            email,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// Find a user by UUID.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, role, created_at
               FROM users WHERE id = $1"#,
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// List all users with optional search/role filter.
    pub async fn list_all(
        &self,
        search: Option<&str>,
        role:   Option<&str>,
        limit:  i64,
        offset: i64,
    ) -> Result<Vec<User>> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT id, email, password_hash, role, created_at FROM users WHERE true",
        );
        if let Some(s) = search {
            qb.push(" AND email ILIKE ").push_bind(format!("%{s}%"));
        }
        if let Some(r) = role {
            qb.push(" AND role = ").push_bind(r.to_string());
        }
        qb.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit)
          .push(" OFFSET ").push_bind(offset);

        qb.build_query_as::<User>()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))
    }

    /// Count users matching optional filters.
    pub async fn count_all(&self, search: Option<&str>, role: Option<&str>) -> Result<i64> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT COUNT(*) FROM users WHERE true",
        );
        if let Some(s) = search {
            qb.push(" AND email ILIKE ").push_bind(format!("%{s}%"));
        }
        if let Some(r) = role {
            qb.push(" AND role = ").push_bind(r.to_string());
        }
        let row: (i64,) = qb.build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(row.0)
    }

    /// Update a user's role.
    pub async fn update_role(&self, id: Uuid, role: &str) -> Result<User> {
        sqlx::query_as::<_, User>(
            "UPDATE users SET role = $1 WHERE id = $2 RETURNING id, email, password_hash, role, created_at"
        )
        .bind(role)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DecentGpuError::Database(e.to_string()))?
        .ok_or_else(|| DecentGpuError::NotFound(format!("user {id} not found")))
    }

    /// Count how many admin users exist.
    pub async fn count_admins(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DecentGpuError::Database(e.to_string()))?;
        Ok(row.0)
    }
}
