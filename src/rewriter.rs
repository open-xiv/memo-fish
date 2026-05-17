//! JSON field-name rewriting to match memo-docs/standards/observability.md.
//!
//! tracing-subscriber's JSON formatter hardcodes `timestamp` and `message`;
//! the cross-language standard says `ts` and `msg` (the Go services rename
//! these via zerolog's `TimestampFieldName` / `MessageFieldName` knobs).
//! Implementing a custom `FormatEvent` would mean owning ~80 lines of
//! careful boilerplate and keeping it in sync with tracing-subscriber's
//! internals; instead, wrap the destination writer and rewrite the two
//! well-known keys on the way out. One serde_json round-trip per event is
//! cheap at our log volumes.

use std::io::{self, Write};

/// MakeWriter that wraps an inner MakeWriter, applying [`Rewriter`] to
/// every produced writer.
pub struct Rewriting<M>(pub M);

impl<'a, M> tracing_subscriber::fmt::MakeWriter<'a> for Rewriting<M>
where
    M: tracing_subscriber::fmt::MakeWriter<'a>,
{
    type Writer = Rewriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        Rewriter {
            inner: self.0.make_writer(),
        }
    }
}

/// Renames `timestamp` → `ts` and `message` → `msg` on the way through.
/// Non-JSON input passes through untouched (e.g. dev-mode human output,
/// though in practice this writer is only attached to the JSON layer).
pub struct Rewriter<W: Write> {
    inner: W,
}

impl<W: Write> Write for Rewriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // tracing-subscriber's JSON formatter writes one complete event per
        // call, terminating in '\n'. Strip the trailing newline before
        // parsing, then re-add it after re-serializing.
        let written = buf.len();
        let trimmed: &[u8] = match buf.last() {
            Some(b'\n') => &buf[..buf.len() - 1],
            _ => buf,
        };
        if trimmed.is_empty() {
            self.inner.write_all(buf)?;
            return Ok(written);
        }
        match serde_json::from_slice::<serde_json::Value>(trimmed) {
            Ok(mut v) => {
                if let serde_json::Value::Object(ref mut m) = v {
                    if let Some(t) = m.remove("timestamp") {
                        m.insert("ts".into(), t);
                    }
                    if let Some(msg) = m.remove("message") {
                        m.insert("msg".into(), msg);
                    }
                    // tracing-subscriber emits `INFO`/`WARN`/etc; the spec
                    // says lowercase to match zerolog. Cheaper to lowercase
                    // in place here than to write a custom event formatter.
                    if let Some(level) = m.get_mut("level") {
                        if let Some(s) = level.as_str() {
                            let lower = s.to_ascii_lowercase();
                            *level = serde_json::Value::String(lower);
                        }
                    }
                }
                serde_json::to_writer(&mut self.inner, &v)?;
                self.inner.write_all(b"\n")?;
            }
            Err(_) => {
                self.inner.write_all(buf)?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
