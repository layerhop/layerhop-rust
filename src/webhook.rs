//! Verify the `LayerHop-Signature` header on webhook deliveries.
//!
//! Header format: `t=<unix seconds>,v1=<hex hmac_sha256(secret, "<t>.<raw body>")>`.
//! Several `v1=` entries may be present (secret rotation); any match is accepted.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

/// Name of the signature header.
pub const SIGNATURE_HEADER: &str = "LayerHop-Signature";
/// Default allowed clock skew / replay window.
pub const DEFAULT_TOLERANCE: Duration = Duration::from_secs(300);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WebhookError {
    #[error("malformed LayerHop-Signature header")]
    MalformedHeader,
    #[error("signature timestamp outside the tolerance window")]
    TimestampOutOfTolerance,
    #[error("no matching signature")]
    SignatureMismatch,
}

/// Verify a webhook delivery. `body` must be the raw request body, byte for byte.
/// `now` is the current time; pass `SystemTime::now()` in production.
pub fn verify_signature(
    secret: &str,
    body: &[u8],
    header: &str,
    tolerance: Duration,
    now: SystemTime,
) -> Result<(), WebhookError> {
    let mut timestamp: Option<&str> = None;
    let mut signatures: Vec<&str> = Vec::new();
    for part in header.split(',') {
        let mut kv = part.trim().splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("t"), Some(v)) => timestamp = Some(v.trim()),
            (Some("v1"), Some(v)) => signatures.push(v.trim()),
            _ => {}
        }
    }
    let t_str = timestamp.ok_or(WebhookError::MalformedHeader)?;
    let t: u64 = t_str.parse().map_err(|_| WebhookError::MalformedHeader)?;
    if signatures.is_empty() {
        return Err(WebhookError::MalformedHeader);
    }
    let now_s = now.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if now_s.abs_diff(t) > tolerance.as_secs() {
        return Err(WebhookError::TimestampOutOfTolerance);
    }
    for sig in signatures {
        let Ok(expected) = hex::decode(sig) else { continue };
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
            .expect("HMAC accepts keys of any length");
        mac.update(t_str.as_bytes());
        mac.update(b".");
        mac.update(body);
        if mac.verify_slice(&expected).is_ok() {
            return Ok(());
        }
    }
    Err(WebhookError::SignatureMismatch)
}

/// [`verify_signature`] with the default 300 s tolerance and the current time.
pub fn verify(secret: &str, body: &[u8], header: &str) -> Result<(), WebhookError> {
    verify_signature(secret, body, header, DEFAULT_TOLERANCE, SystemTime::now())
}

/// Compute a `LayerHop-Signature` header value (useful for tests and local tooling).
pub fn sign(secret: &str, body: &[u8], timestamp: u64) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    format!("t={},v1={}", timestamp, hex::encode(mac.finalize().into_bytes()))
}
