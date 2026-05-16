use tokio::signal;
use tokio::sync::broadcast;

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
