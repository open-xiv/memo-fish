use chrono::{DateTime, Utc};
use std::sync::OnceLock;

pub const SERVICE: &str = "memo-fish";

/// MEMO_VERSION is injected at image-build time as a docker ARG/ENV; falls back to
/// "dev" for local cargo run.
pub fn version() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| std::env::var("MEMO_VERSION").unwrap_or_else(|_| "dev".into()))
}

/// short SHA injected the same way as version; "unknown" when missing.
pub fn build() -> &'static str {
    static B: OnceLock<String> = OnceLock::new();
    B.get_or_init(|| std::env::var("MEMO_BUILD").unwrap_or_else(|_| "unknown".into()))
}

pub fn env_label() -> &'static str {
    static E: OnceLock<String> = OnceLock::new();
    E.get_or_init(|| std::env::var("MEMO_ENV").unwrap_or_else(|_| "dev".into()))
}

pub fn started_at() -> DateTime<Utc> {
    static T: OnceLock<DateTime<Utc>> = OnceLock::new();
    *T.get_or_init(Utc::now)
}
