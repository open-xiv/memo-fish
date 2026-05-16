use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

mod api;
mod buildinfo;
mod config;
mod logger;
mod shutdown;
mod writer;

use writer::{Record, WriterMetrics};

#[derive(Clone)]
pub struct AppState {
    pub tx: mpsc::Sender<Record>,
    pub ingest_key: Arc<String>,
    pub download_key: Arc<String>,
    pub data_dir: Arc<PathBuf>,
    pub writer_metrics: Arc<WriterMetrics>,
    pub queue_cap: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::Config::from_env()?;
    let root_span = logger::init(&cfg);
    let _root_guard = root_span.entered();
    let _ = buildinfo::started_at();

    tokio::fs::create_dir_all(&cfg.data_dir).await?;

    let (tx, rx) = mpsc::channel::<Record>(cfg.queue_cap);
    let writer_metrics = WriterMetrics::new();
    let writer_handle = tokio::spawn(writer::run(
        rx,
        cfg.data_dir.clone(),
        cfg.retention_days,
        writer_metrics.clone(),
    ));

    let shutdown = shutdown::Shutdown::new();

    let state = AppState {
        tx,
        ingest_key: Arc::new(cfg.ingest_key.clone()),
        download_key: Arc::new(cfg.download_key.clone()),
        data_dir: Arc::new(cfg.data_dir.clone()),
        writer_metrics: writer_metrics.clone(),
        queue_cap: cfg.queue_cap,
    };

    let public_task = tokio::spawn(api::serve_public(
        cfg.public_bind.clone(),
        state.clone(),
        shutdown.subscribe(),
    ));

    let metrics_task = if cfg.metrics_bind.is_empty() {
        tracing::info!("metrics listener disabled (MEMO_FISH_METRICS_BIND empty)");
        None
    } else {
        Some(tokio::spawn(api::serve_metrics(
            cfg.metrics_bind.clone(),
            state.clone(),
            shutdown.subscribe(),
        )))
    };

    // drop the local handle so only the two router-held copies (and writer-held tx) remain;
    // once both servers exit, the only path to keep the tx alive is through the writer's
    // own pair, which is its rx, not a tx — so writer's rx.recv() will see channel close.
    drop(state);

    shutdown::wait_for_signal().await;
    tracing::info!(event = "shutdown.signal", "shutdown signal received, draining");
    shutdown.trigger();

    if let Err(e) = public_task.await {
        tracing::warn!(error = %e, "public server task join error");
    }
    if let Some(h) = metrics_task {
        if let Err(e) = h.await {
            tracing::warn!(error = %e, "metrics server task join error");
        }
    }

    if let Err(e) = writer_handle.await {
        tracing::warn!(error = %e, "writer task join error");
    }

    tracing::info!(event = "shutdown.done", "bye");
    Ok(())
}
