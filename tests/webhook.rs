use std::time::{Duration, UNIX_EPOCH};

use layerhop::webhook::{sign, verify_signature, WebhookError, DEFAULT_TOLERANCE};

const SECRET: &str = "whsec_test";
const BODY: &[u8] = br#"{"id":"evt_1","type":"order.filled"}"#;
const T: u64 = 1_700_000_000;
// hmac_sha256("whsec_test", "1700000000.<BODY>") computed independently.
const KNOWN_SIG: &str = "79513d43f7d4591eb0a5c0e31b61880c6eec5af78e46f5740af9e8e946c9fc33";

fn at(secs: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn sign_matches_known_vector() {
    assert_eq!(sign(SECRET, BODY, T), format!("t={T},v1={KNOWN_SIG}"));
}

#[test]
fn accepts_valid_signature() {
    let header = format!("t={T},v1={KNOWN_SIG}");
    assert_eq!(verify_signature(SECRET, BODY, &header, DEFAULT_TOLERANCE, at(T + 10)), Ok(()));
}

#[test]
fn accepts_any_of_multiple_v1_and_spaces() {
    let header = format!("t={T}, v1={}, v1={KNOWN_SIG}", "00".repeat(32));
    assert_eq!(verify_signature(SECRET, BODY, &header, DEFAULT_TOLERANCE, at(T)), Ok(()));
}

#[test]
fn rejects_tampered_body() {
    let header = format!("t={T},v1={KNOWN_SIG}");
    assert_eq!(
        verify_signature(SECRET, b"{}", &header, DEFAULT_TOLERANCE, at(T)),
        Err(WebhookError::SignatureMismatch)
    );
}

#[test]
fn rejects_wrong_secret() {
    let header = format!("t={T},v1={KNOWN_SIG}");
    assert_eq!(
        verify_signature("other", BODY, &header, DEFAULT_TOLERANCE, at(T)),
        Err(WebhookError::SignatureMismatch)
    );
}

#[test]
fn rejects_old_and_future_timestamps() {
    let header = format!("t={T},v1={KNOWN_SIG}");
    assert_eq!(
        verify_signature(SECRET, BODY, &header, DEFAULT_TOLERANCE, at(T + 301)),
        Err(WebhookError::TimestampOutOfTolerance)
    );
    assert_eq!(
        verify_signature(SECRET, BODY, &header, DEFAULT_TOLERANCE, at(T - 301)),
        Err(WebhookError::TimestampOutOfTolerance)
    );
    assert_eq!(verify_signature(SECRET, BODY, &header, DEFAULT_TOLERANCE, at(T + 300)), Ok(()));
}

#[test]
fn rejects_malformed_headers() {
    let only_t = format!("t={T}");
    let headers: [&str; 4] = ["", "v1=abc", "t=notanumber,v1=abc", only_t.as_str()];
    for h in headers {
        assert_eq!(
            verify_signature(SECRET, BODY, h, DEFAULT_TOLERANCE, at(T)),
            Err(WebhookError::MalformedHeader),
            "header {h:?}"
        );
    }
}

#[test]
fn non_hex_signature_is_mismatch() {
    let header = format!("t={T},v1=zzzz");
    assert_eq!(
        verify_signature(SECRET, BODY, &header, DEFAULT_TOLERANCE, at(T)),
        Err(WebhookError::SignatureMismatch)
    );
}
