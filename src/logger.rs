use crate::buildinfo;
use crate::config::{Config, Env};
use crate::rewriter::Rewriting;
use tracing::Span;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize tracing. Always writes JSON to stdout (fly captures that for
/// `flyctl logs`). If `MEMO_FISH_LOG_DIR` is set, also writes the same JSON
/// to a rolling file in that directory — the in-container alloy sidecar
/// tails the file and ships to the central Loki via mesh0.
///
/// The returned `Option<WorkerGuard>` is the non-blocking writer's guard;
/// callers MUST hold it for the lifetime of the program or the background
/// flush thread will be dropped and the tail of the log silently lost.
pub fn init(cfg: &Config) -> (Span, Option<WorkerGuard>) {
    let filter = EnvFilter::try_from_env("LOG_LEVEL")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let mut guard: Option<WorkerGuard> = None;

    match cfg.memo_env {
        Env::Prod => {
            // The stdout JSON layer goes through Rewriting so `timestamp` /
            // `message` come out as `ts` / `msg` per the observability spec.
            let stdout_layer = fmt::layer()
                .json()
                .flatten_event(true)
                .with_target(false)
                .with_current_span(true)
                .with_span_list(false)
                .with_writer(Rewriting(std::io::stdout));

            let file_layer = std::env::var("MEMO_FISH_LOG_DIR").ok().map(|dir| {
                std::fs::create_dir_all(&dir).expect("create MEMO_FISH_LOG_DIR");
                let appender = tracing_appender::rolling::Builder::new()
                    .rotation(tracing_appender::rolling::Rotation::DAILY)
                    .filename_prefix("app")
                    .filename_suffix("log")
                    .max_log_files(2)
                    .build(&dir)
                    .expect("build rolling file appender");
                let (nb, g) = tracing_appender::non_blocking(appender);
                guard = Some(g);
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_target(false)
                    .with_current_span(true)
                    .with_span_list(false)
                    .with_writer(Rewriting(nb))
            });

            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
        }
        Env::Dev => tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(false))
            .init(),
    }

    let span = tracing::info_span!(
        "root",
        service = buildinfo::SERVICE,
        version = buildinfo::version(),
        build = buildinfo::build(),
        env = buildinfo::env_label(),
    );
    (span, guard)
}
