use tokio::signal;
use tokio::sync::broadcast;

/// fan-out shutdown signal. main subscribes once per long-lived task; each task should
/// race its own work against `rx.recv()` and exit cleanly when the signal fires.
#[derive(Clone)]
pub struct Shutdown(pub broadcast::Sender<()>);

impl Shutdown {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self(tx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.0.subscribe()
    }

    pub fn trigger(&self) {
        let _ = self.0.send(());
    }
}

/// resolves when the process should begin draining. SIGTERM (fly stop), SIGINT (ctrl-c).
pub async fn wait_for_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let term = async {
        use signal::unix::{signal as unix_signal, SignalKind};
        if let Ok(mut s) = unix_signal(SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
}
