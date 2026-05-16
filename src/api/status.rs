use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;

use crate::buildinfo;
use crate::writer::WEDGED_THRESHOLD;
use crate::AppState;

/// minimum free bytes on /data before the volume check trips. picked to give the
/// 7-day retention pruner a chance to free space at the next rotation while still
/// flagging "you're about to run out" before writes start failing.
const MIN_VOLUME_FREE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Serialize)]
struct Check {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct StatusBody {
    service: &'static str,
    version: &'static str,
    build: &'static str,
    env: &'static str,
    started_at: DateTime<Utc>,
    uptime_seconds: i64,
    status: &'static str,
    checks: BTreeMap<&'static str, Check>,
}

pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let writer = writer_check(&state);
    let volume = volume_check(&state);
    let overall_ok = writer.ok && volume.ok;
    let overall = if overall_ok { "ok" } else { "down" };
    let code = if overall_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let mut checks = BTreeMap::new();
    checks.insert("writer", writer);
    checks.insert("volume", volume);
    let body = StatusBody {
        service: buildinfo::SERVICE,
        version: buildinfo::version(),
        build: buildinfo::build(),
        env: buildinfo::env_label(),
        started_at: buildinfo::started_at(),
        uptime_seconds: (Utc::now() - buildinfo::started_at()).num_seconds(),
        status: overall,
        checks,
    };
    (code, Json(body))
}

pub async fn live() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let writer = writer_check(&state);
    let volume = volume_check(&state);
    if writer.ok && volume.ok {
        (StatusCode::OK, Json(json!({ "status": "ok" })))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "down" })),
        )
    }
}

fn writer_check(state: &AppState) -> Check {
    let last = state.writer_metrics.last_tick_ms.load(Ordering::Relaxed);
    let now = Utc::now().timestamp_millis();
    let age = Duration::from_millis((now - last).max(0) as u64);
    if state.tx.is_closed() {
        return Check {
            ok: false,
            latency_ms: None,
            error: Some("writer channel closed".into()),
        };
    }
    if age > WEDGED_THRESHOLD {
        return Check {
            ok: false,
            latency_ms: Some(age.as_millis() as i64),
            error: Some(format!("writer last tick {}s ago", age.as_secs())),
        };
    }
    Check {
        ok: true,
        latency_ms: Some(age.as_millis() as i64),
        error: None,
    }
}

fn volume_check(state: &AppState) -> Check {
    match super::volume_free_bytes(&state.data_dir) {
        Ok(free) if free >= MIN_VOLUME_FREE_BYTES => Check {
            ok: true,
            latency_ms: None,
            error: None,
        },
        Ok(free) => Check {
            ok: false,
            latency_ms: None,
            error: Some(format!("free bytes {free} below threshold {MIN_VOLUME_FREE_BYTES}")),
        },
        Err(e) => Check {
            ok: false,
            latency_ms: None,
            error: Some(e.to_string()),
        },
    }
}
