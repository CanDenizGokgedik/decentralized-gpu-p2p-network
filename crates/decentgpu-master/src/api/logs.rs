//! SSE log streaming endpoint.

use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
    Extension,
};
use std::convert::Infallible;
use uuid::Uuid;

use super::{auth::ApiError, auth::AuthUser, AppState};

/// `GET /api/v1/jobs/:id/logs` — stream job logs as Server-Sent Events.
pub async fn stream_logs(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
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

    // Fetch all log lines from the beginning (since_ms = 0).
    let logs = state
        .db
        .jobs
        .get_logs_since(id, 0, 10_000)
        .await?;

    let events: Vec<Result<Event, Infallible>> = logs
        .into_iter()
        .map(|(timestamp_ms, level, message)| {
            let data = serde_json::json!({
                "timestamp_ms": timestamp_ms,
                "level":        level,
                "message":      message,
            });
            Ok(Event::default().data(data.to_string()))
        })
        .collect();

    let stream = tokio_stream::iter(events);

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    ))
}
