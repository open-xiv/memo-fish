use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::time::interval;

const BATCH_BYTES: usize = 32 * 1024;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub struct Incoming {
    pub v: [f32; 5],
}

#[derive(Debug, Serialize)]
pub struct Record {
    /// unix millis, UTC, server-stamped at /ingest enqueue time.
    pub ts: i64,
    pub v: [f32; 5],
}

/// counters and liveness exposed to /status + /metrics. writer updates these from one
/// task; readers (axum handlers) read them with relaxed ordering — exact-instant
/// freshness is not required for monitoring.
pub struct WriterMetrics {
    pub records_written: AtomicU64,
    pub write_errors: AtomicU64,
    /// unix millis of the writer's last loop tick. /status/ready considers the writer
    /// wedged if this is older than WEDGED_THRESHOLD.
    pub last_tick_ms: AtomicI64,
}

impl WriterMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            records_written: AtomicU64::new(0),
            write_errors: AtomicU64::new(0),
            last_tick_ms: AtomicI64::new(Utc::now().timestamp_millis()),
        })
    }

    pub fn tick(&self) {
        self.last_tick_ms
            .store(Utc::now().timestamp_millis(), Ordering::Relaxed);
    }
}

/// the writer is considered wedged if it hasn't ticked for this long. tick happens on
/// every channel receive AND on every 5s flush interval, so 30s without a tick implies
/// the task is blocked or panicked.
pub const WEDGED_THRESHOLD: Duration = Duration::from_secs(30);

pub async fn run(
    mut rx: mpsc::Receiver<Record>,
    data_dir: PathBuf,
    retention_days: u32,
    metrics: Arc<WriterMetrics>,
) {
    if let Err(e) = tokio::fs::create_dir_all(&data_dir).await {
        tracing::error!(event = "writer.startup_failed", error = %e, dir = %data_dir.display(), "create data dir failed");
        return;
    }
    prune_old(&data_dir, retention_days).await;

    let mut current_date: Option<NaiveDate> = None;
    let mut file: Option<File> = None;
    let mut buf: Vec<u8> = Vec::with_capacity(BATCH_BYTES * 2);
    let mut tick = interval(FLUSH_INTERVAL);

    loop {
        tokio::select! {
            biased;
            maybe = rx.recv() => {
                metrics.tick();
                let Some(rec) = maybe else {
                    // tx side dropped — main is shutting down. final flush + fsync then exit.
                    flush(&mut file, &mut buf, true, &metrics).await;
                    tracing::info!(event = "writer.drained", "writer task exiting on channel close");
                    return;
                };

                let today = Utc::now().date_naive();
                if Some(today) != current_date {
                    flush(&mut file, &mut buf, true, &metrics).await;
                    match open_day(&data_dir, today).await {
                        Ok(f) => {
                            file = Some(f);
                            let rotated_from = current_date;
                            current_date = Some(today);
                            tracing::info!(event = "rotate.success", date = %today, "rotated to new day");
                            if rotated_from.is_some() {
                                prune_old(&data_dir, retention_days).await;
                            }
                        }
                        Err(e) => {
                            tracing::error!(event = "rotate.failed", error = %e, date = %today, "failed to open day file");
                            metrics.write_errors.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    }
                }

                if let Ok(mut line) = serde_json::to_string(&rec) {
                    line.push('\n');
                    buf.extend_from_slice(line.as_bytes());
                }

                if buf.len() >= BATCH_BYTES {
                    flush(&mut file, &mut buf, false, &metrics).await;
                }
            }
            _ = tick.tick() => {
                metrics.tick();
                flush(&mut file, &mut buf, true, &metrics).await;
            }
        }
    }
}

async fn flush(
    file: &mut Option<File>,
    buf: &mut Vec<u8>,
    fsync: bool,
    metrics: &WriterMetrics,
) {
    if buf.is_empty() {
        return;
    }
    let Some(f) = file.as_mut() else {
        // no file yet (first record before rotate completed) — keep buffered.
        return;
    };
    // count records by counting newlines so a partial write is not double-counted.
    let lines = buf.iter().filter(|b| **b == b'\n').count() as u64;
    if let Err(e) = f.write_all(buf).await {
        tracing::error!(event = "write.failed", error = %e, bytes = buf.len(), "appending to day file failed");
        metrics.write_errors.fetch_add(1, Ordering::Relaxed);
        return;
    }
    buf.clear();
    metrics.records_written.fetch_add(lines, Ordering::Relaxed);
    if fsync {
        if let Err(e) = f.sync_data().await {
            tracing::warn!(event = "fsync.failed", error = %e, "sync_data failed; data is in page cache");
        }
    }
}

async fn open_day(data_dir: &Path, date: NaiveDate) -> std::io::Result<File> {
    let path = data_dir.join(format!("data-{}.jsonl", date));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
}

/// removes data-YYYY-MM-DD.jsonl files whose date is strictly older than retention_days.
/// today's file is never touched. errors are logged at warn and otherwise ignored — the
/// next rotation will retry.
async fn prune_old(data_dir: &Path, retention_days: u32) {
    let cutoff = Utc::now().date_naive() - chrono::Days::new(retention_days as u64);
    let mut rd = match tokio::fs::read_dir(data_dir).await {
        Ok(rd) => rd,
        Err(e) => {
            tracing::warn!(event = "prune.failed", error = %e, "read_dir failed");
            return;
        }
    };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        let Some(date) = parse_day_filename(name_str) else { continue };
        if date < cutoff {
            match tokio::fs::remove_file(entry.path()).await {
                Ok(_) => tracing::info!(event = "prune.deleted", file = name_str, date = %date, "pruned old day file"),
                Err(e) => tracing::warn!(event = "prune.failed", error = %e, file = name_str, "remove_file failed"),
            }
        }
    }
}

/// "data-2026-05-16.jsonl" -> Some(2026-05-16); anything else -> None.
fn parse_day_filename(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_prefix("data-")?.strip_suffix(".jsonl")?;
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}
