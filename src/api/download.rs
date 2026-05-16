use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;

use crate::AppState;

use super::auth;

pub async fn download(
    State(state): State<AppState>,
    Path(date): Path<String>,
    headers: HeaderMap,
) -> Response {
    if NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if !auth::check_key(&headers, &state.download_key) {
        tracing::debug!(event = "download.unauthorized", date = %date, "missing or wrong x-auth-key");
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let path = state.data_dir.join(format!("data-{}.jsonl", date));
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            tracing::debug!(event = "download.success", date = %date, bytes = bytes.len(), "served day file");
            (
                [(header::CONTENT_TYPE, "application/x-ndjson")],
                bytes,
            )
                .into_response()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(event = "download.notfound", date = %date, "no file for date");
            StatusCode::NOT_FOUND.into_response()
        }
        Err(e) => {
            tracing::error!(event = "download.read_failed", error = %e, date = %date, "read failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
