use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use tokio::sync::mpsc::error::TrySendError;

use crate::writer::{Incoming, Record};
use crate::AppState;

use super::auth;

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<Incoming>,
) -> StatusCode {
    if !auth::check_key(&headers, &state.ingest_key) {
        tracing::debug!(event = "ingest.unauthorized", "missing or wrong x-auth-key");
        return StatusCode::UNAUTHORIZED;
    }
    let rec = Record {
        ts: Utc::now().timestamp_millis(),
        v: p.v,
    };
    match state.tx.try_send(rec) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(TrySendError::Full(_)) => {
            tracing::warn!(event = "ingest.busy", queue_cap = state.queue_cap, "channel full");
            StatusCode::TOO_MANY_REQUESTS
        }
        Err(TrySendError::Closed(_)) => {
            tracing::error!(event = "ingest.closed", "writer channel closed");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}
