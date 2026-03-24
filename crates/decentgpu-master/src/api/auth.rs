//! Authentication endpoints and JWT middleware.

use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{
    extract::{Query, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;

// ─── Request / Response DTOs ─────────────────────────────────────────────────

/// Registration request body.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email:    String,
    pub password: String,
    pub role:     String,
}

/// Login request body.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email:    String,
    pub password: String,
}

/// Authentication response (returned on register and login).
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token:      String,
    pub user_id:    Uuid,
    pub email:      String,
    pub role:       String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cu_balance: Option<i64>,
}

// ─── JWT Claims ───────────────────────────────────────────────────────────────

/// JWT payload.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject: user UUID.
    pub sub:   String,
    /// User email (for convenience — not authoritative).
    #[serde(default)]
    pub email: String,
    /// User role.
    pub role:  String,
    /// Expiry (Unix timestamp).
    pub exp:   usize,
}

/// Axum extension injected by the JWT middleware.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub email:   String,
    pub role:    String,
}

#[async_trait::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, ApiError> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| ApiError::unauthorized("not authenticated"))
    }
}

/// Extractor for admin-only routes — fails if the user's role is not `"admin"`.
#[derive(Debug, Clone)]
pub struct AdminUser {
    pub user_id: Uuid,
}

#[async_trait::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, ApiError> {
        let user = parts
            .extensions
            .get::<AuthUser>()
            .ok_or_else(|| ApiError::unauthorized("not authenticated"))?;
        if user.role != "admin" {
            return Err(ApiError::forbidden("admin role required"));
        }
        Ok(AdminUser { user_id: user.user_id })
    }
}

/// Query param for token-based auth (used by WebSocket upgrades).
#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

// ─── Validation helpers ───────────────────────────────────────────────────────

/// Validate email format: must contain `@` with non-empty local and domain parts
/// where the domain contains at least one dot.
pub fn validate_email(email: &str) -> bool {
    let mut parts = email.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

/// Password strength: minimum 8 characters, at least one uppercase letter,
/// one lowercase letter, and one ASCII digit.
pub fn validate_password(password: &str) -> bool {
    password.len() >= 8
        && password.chars().any(|c| c.is_uppercase())
        && password.chars().any(|c| c.is_lowercase())
        && password.chars().any(|c| c.is_ascii_digit())
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `POST /api/v1/auth/register`
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if !validate_email(&body.email) {
        return Err(ApiError::bad_request("invalid email address"));
    }
    if !validate_password(&body.password) {
        return Err(ApiError::bad_request(
            "password must be at least 8 characters with uppercase, lowercase, and a digit",
        ));
    }
    let role = body.role.as_str();
    if !matches!(role, "hirer" | "worker" | "both") {
        return Err(ApiError::bad_request("role must be hirer, worker, or both"));
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(body.password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(format!("hash error: {e}")))?
        .to_string();

    let user = state.db.users.create(&body.email, &hash, role).await?;
    state.ledger.initialize(user.id).await?;

    let (token, expires_at) = mint_token(&state, user.id, &user.email, role)?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user_id:    user.id,
            email:      user.email,
            role:       role.to_string(),
            expires_at,
            cu_balance: None,
        }),
    ))
}

/// `POST /api/v1/auth/login`
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user = state
        .db
        .users
        .find_by_email(&body.email)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invalid credentials"))?;

    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|e| ApiError::internal(format!("invalid stored hash: {e}")))?;
    Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed)
        .map_err(|_| ApiError::unauthorized("invalid credentials"))?;

    let (token, expires_at) = mint_token(&state, user.id, &user.email, &user.role)?;

    let cu_balance = state.ledger.get_balance(user.id).await.ok().map(|b| b.cu_balance);

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        email:   user.email,
        role:    user.role,
        expires_at,
        cu_balance,
    }))
}

/// `GET /api/v1/auth/me`
pub async fn me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> Result<impl IntoResponse, ApiError> {
    let user = state
        .db
        .users
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    let balance = state.ledger.get_balance(user.id).await.ok().map(|b| b.cu_balance);

    Ok(Json(serde_json::json!({
        "user_id":    user.id,
        "email":      user.email,
        "role":       user.role,
        "created_at": user.created_at,
        "cu_balance": balance,
    })))
}

// ─── JWT Middleware ────────────────────────────────────────────────────────────

/// Axum middleware that validates a Bearer JWT.
///
/// Checks `Authorization: Bearer <token>` first; falls back to `?token=` query
/// param (needed for WebSocket upgrades from browsers).
pub async fn jwt_middleware(
    State(state): State<AppState>,
    Query(q): Query<TokenQuery>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let raw = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_owned())
        .or(q.token)
        .ok_or_else(|| ApiError::unauthorized("missing Bearer token"))?;

    let claims = decode::<Claims>(
        &raw,
        &DecodingKey::from_secret(&state.jwt_secret),
        &Validation::default(),
    )
    .map_err(|e| ApiError::unauthorized(format!("invalid token: {e}")))?
    .claims;

    let user_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| ApiError::unauthorized("invalid sub in token"))?;

    req.extensions_mut().insert(AuthUser {
        user_id,
        email: claims.email,
        role:  claims.role,
    });

    Ok(next.run(req).await)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Mint a JWT and return `(token_string, expires_at_rfc3339)`.
fn mint_token(
    state:   &AppState,
    user_id: Uuid,
    email:   &str,
    role:    &str,
) -> Result<(String, String), ApiError> {
    let exp = (Utc::now().timestamp() as usize) + state.jwt_expiry_secs as usize;
    let claims = Claims {
        sub:   user_id.to_string(),
        email: email.to_string(),
        role:  role.to_string(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&state.jwt_secret),
    )
    .map_err(|e| ApiError::internal(format!("token encode error: {e}")))?;

    let expires_at = (Utc::now() + chrono::Duration::seconds(state.jwt_expiry_secs as i64))
        .to_rfc3339();

    Ok((token, expires_at))
}

// ─── Error type ───────────────────────────────────────────────────────────────

/// API error that implements [`IntoResponse`].
#[derive(Debug)]
pub struct ApiError {
    status:  StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: msg.into() }
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::UNAUTHORIZED, message: msg.into() }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: msg.into() }
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::FORBIDDEN, message: msg.into() }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: msg.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => Self::not_found("record not found"),
            other => {
                tracing::error!(error = %other, "database error");
                Self::internal("database error")
            }
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "internal error");
        Self::internal(e.to_string())
    }
}

impl From<decentgpu_common::DecentGpuError> for ApiError {
    fn from(e: decentgpu_common::DecentGpuError) -> Self {
        use decentgpu_common::DecentGpuError;
        match e {
            DecentGpuError::NotFound(m) => Self::not_found(m),
            DecentGpuError::Unauthorized(m) => Self::unauthorized(m),
            DecentGpuError::Validation(m) => Self::bad_request(m),
            DecentGpuError::Conflict(m) => Self::bad_request(m),
            DecentGpuError::InsufficientComputeUnits { available, required } => {
                Self::bad_request(format!(
                    "insufficient compute units: have {available}, need {required}"
                ))
            }
            other => Self::internal(other.to_string()),
        }
    }
}
