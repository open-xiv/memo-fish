use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub memo_env: Env,
    pub data_dir: PathBuf,
    pub public_bind: String,
    pub metrics_bind: String,
    pub queue_cap: usize,
    pub retention_days: u32,
    pub ingest_key: String,
    pub download_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Dev,
    Prod,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let memo_env = match env::var("MEMO_ENV").as_deref().unwrap_or("dev") {
            "prod" => Env::Prod,
            _ => Env::Dev,
        };
        Ok(Self {
            memo_env,
            data_dir: env::var("MEMO_FISH_DATA_DIR")
                .unwrap_or_else(|_| "/data".into())
                .into(),
            public_bind: env::var("MEMO_FISH_PUBLIC_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8080".into()),
            metrics_bind: env::var("MEMO_FISH_METRICS_BIND").unwrap_or_default(),
            queue_cap: parse_or("MEMO_FISH_QUEUE_CAP", 10_000)?,
            retention_days: parse_or("MEMO_FISH_RETENTION_DAYS", 7u32)?,
            ingest_key: required("MEMO_FISH_INGEST_KEY")?,
            download_key: required("MEMO_FISH_DOWNLOAD_KEY")?,
        })
    }
}

fn required(key: &str) -> Result<String> {
    let v = env::var(key).with_context(|| format!("{key} not set"))?;
    if v.is_empty() {
        anyhow::bail!("{key} is empty");
    }
    Ok(v)
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(v) if !v.is_empty() => v
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("{key} invalid: {e}")),
        _ => Ok(default),
    }
}
