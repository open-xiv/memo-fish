use std::fmt::Write;
use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderMap;
use chrono::Utc;

use crate::buildinfo;
use crate::AppState;

pub async fn metrics(State(state): State<AppState>) -> (HeaderMap, String) {
    let mut buf = String::with_capacity(2048);
    let labels = format!(
        r#"{{service="{}",version="{}",build="{}",env="{}"}}"#,
        buildinfo::SERVICE,
        buildinfo::version(),
        buildinfo::build(),
        buildinfo::env_label(),
    );

    let uptime = (Utc::now() - buildinfo::started_at()).num_seconds().max(0);
    let _ = writeln!(buf, "# HELP fish_uptime_seconds wall-clock seconds since process start");
    let _ = writeln!(buf, "# TYPE fish_uptime_seconds gauge");
    let _ = writeln!(buf, "fish_uptime_seconds{labels} {uptime}");

    let records = state.writer_metrics.records_written.load(Ordering::Relaxed);
    let _ = writeln!(buf, "# HELP fish_records_written_total records flushed to disk");
    let _ = writeln!(buf, "# TYPE fish_records_written_total counter");
    let _ = writeln!(buf, "fish_records_written_total{labels} {records}");

    let errors = state.writer_metrics.write_errors.load(Ordering::Relaxed);
    let _ = writeln!(buf, "# HELP fish_write_errors_total write/rotate errors observed by the writer task");
    let _ = writeln!(buf, "# TYPE fish_write_errors_total counter");
    let _ = writeln!(buf, "fish_write_errors_total{labels} {errors}");

    let depth = state
        .tx
        .max_capacity()
        .saturating_sub(state.tx.capacity());
    let _ = writeln!(buf, "# HELP fish_queue_depth records buffered in the ingest channel");
    let _ = writeln!(buf, "# TYPE fish_queue_depth gauge");
    let _ = writeln!(buf, "fish_queue_depth{labels} {depth}");

    let _ = writeln!(buf, "# HELP fish_queue_capacity total slots in the ingest channel");
    let _ = writeln!(buf, "# TYPE fish_queue_capacity gauge");
    let _ = writeln!(buf, "fish_queue_capacity{labels} {}", state.queue_cap);

    let free = super::volume_free_bytes(&state.data_dir).unwrap_or(0);
    let _ = writeln!(buf, "# HELP fish_volume_bytes_free bytes available to non-root on the data volume");
    let _ = writeln!(buf, "# TYPE fish_volume_bytes_free gauge");
    let _ = writeln!(buf, "fish_volume_bytes_free{labels} {free}");

    let last_tick = state.writer_metrics.last_tick_ms.load(Ordering::Relaxed);
    let writer_age_ms = (Utc::now().timestamp_millis() - last_tick).max(0);
    let _ = writeln!(buf, "# HELP fish_writer_last_tick_age_ms ms since the writer task last looped");
    let _ = writeln!(buf, "# TYPE fish_writer_last_tick_age_ms gauge");
    let _ = writeln!(buf, "fish_writer_last_tick_age_ms{labels} {writer_age_ms}");

    let mut h = HeaderMap::new();
    h.insert(CONTENT_TYPE, "text/plain; version=0.0.4".parse().unwrap());
    (h, buf)
}
