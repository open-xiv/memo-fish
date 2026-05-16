use std::path::Path;

use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::AppState;

pub mod auth;
pub mod download;
pub mod ingest;
pub mod metrics;
pub mod status;

pub async fn serve_public(
    addr: String,
    state: AppState,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/ingest", post(ingest::ingest))
        .route("/download/:date", get(download::download))
        .route("/status", get(status::status))
        .route("/status/live", get(status::live))
        .route("/status/ready", get(status::ready))
        .with_state(state);

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(addr, "public listener up");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.recv().await;
            tracing::info!("public listener shutting down");
        })
        .await?;
    Ok(())
}

pub async fn serve_metrics(
    addr: String,
    state: AppState,
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics::metrics))
        .with_state(state);

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(addr, "metrics listener up");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.recv().await;
            tracing::info!("metrics listener shutting down");
        })
        .await?;
    Ok(())
}

pub fn volume_free_bytes(path: &Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut s) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
}
