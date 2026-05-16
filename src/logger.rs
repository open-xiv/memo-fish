use crate::buildinfo;
use crate::config::{Config, Env};
use tracing::Span;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// initializes the global subscriber and returns a never-exited root span that carries
/// the baked-in `service` / `version` / `build` / `env` fields. caller should `.entered()`
/// the returned span at the start of `main` and keep the guard alive for the run.
pub fn init(cfg: &Config) -> Span {
    let filter = EnvFilter::try_from_env("LOG_LEVEL")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    match cfg.memo_env {
        Env::Prod => tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_target(false)
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .init(),
        Env::Dev => tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(false))
            .init(),
    }

    tracing::info_span!(
        "root",
        service = buildinfo::SERVICE,
        version = buildinfo::version(),
        build = buildinfo::build(),
        env = buildinfo::env_label(),
    )
}
