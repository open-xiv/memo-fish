use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

/// constant-time compare of the X-Auth-Key header against the expected key.
/// matches memo-server's middleware/auth.go shape — same header name, same primitive.
pub fn check_key(headers: &HeaderMap, expected: &str) -> bool {
    let provided = headers
        .get("x-auth-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}
