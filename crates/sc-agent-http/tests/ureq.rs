//! `UreqTransport` over loopback (REQ-SAG-0402, ADR-SAG-0026 HTTP half).
//!
//! Needs `--features test-util` (`[[test]] required-features`):
//! `Endpoint::loopback_http` and `UreqTransport::loopback_for_tests` let
//! the real ureq agent talk plain HTTP to a std `TcpListener` on 127.0.0.1.

mod support;

use std::sync::Arc;
use std::time::Duration;

use sc_agent_http::test_util::{FakeClock, RecordingSleeper, RecordingTransport};
use sc_agent_http::{
    BusyPolicy, Endpoint, HeaderName, HttpRequest, Method, SecretHeader, Timeouts, Transport,
    UreqTransport, send_with_busy_retry,
};
use sc_agent_types::{BackendKeyId, CancelToken, JevError, NetworkErrorKind, ResolvedApiKey};
use support::{LoopbackServer, Reply, refused_port};

const KEY: &str = "sk-loopback-secret";

/// The response cap of `UreqTransport` (8 MiB).
const CAP: usize = 8 * 1024 * 1024;

fn key() -> ResolvedApiKey {
    ResolvedApiKey::try_new(BackendKeyId::Anthropic, KEY.to_owned()).expect("key")
}

fn loopback() -> UreqTransport {
    UreqTransport::loopback_for_tests(Timeouts::new(
        Duration::from_secs(2),
        Duration::from_secs(10),
    ))
}

fn post(endpoint: Endpoint, secret: Option<SecretHeader>) -> HttpRequest {
    let request = HttpRequest::new(Method::Post, endpoint, Arc::from(&br#"{"q":1}"#[..]))
        .with_header(HeaderName::new("content-type"), "application/json");
    match secret {
        Some(secret) => request.with_secret(secret),
        None => request,
    }
}

fn api_key_header() -> SecretHeader {
    SecretHeader::new(HeaderName::new("x-api-key"), key())
}

fn assert_no_key(error: &JevError) {
    let rendered = format!("{error} {error:?} {}", error.fix_hint());
    assert!(!rendered.contains(KEY), "{rendered}");
}

/// Plain HTTP is refused by `Endpoint::parse` in every build and by the
/// production transport even for a `test-util` loopback endpoint.
#[test]
fn test_release_build_refuses_http() {
    for url in ["http://127.0.0.1:9/x", "http://api.openai.com/v1/responses"] {
        let error = Endpoint::parse(url).expect_err(url);
        assert_eq!(error.code(), "CONFIG_INVALID");
        assert!(error.to_string().contains("https"), "{error}");
    }
    let server = LoopbackServer::start(Vec::new());
    let error = UreqTransport::new(Timeouts::default())
        .send(&post(server.endpoint("/x"), Some(api_key_header())))
        .expect_err("cleartext");
    assert_eq!(error.code(), "NETWORK_ERROR");
    assert!(error.to_string().contains("not https"), "{error}");
    assert_no_key(&error);
    assert!(server.finish().is_empty(), "nothing reached the server");
}

#[test]
fn test_sends_method_headers_secret_body_and_user_agent() {
    let server = LoopbackServer::start(vec![
        Reply::status(200, &[("Retry-After", "2")], b"{\"ok\":1}"),
        Reply::status(200, &[], b"{}"),
    ]);
    let transport = loopback();
    let response = transport
        .send(&post(
            server.endpoint("/v1/messages"),
            Some(api_key_header()),
        ))
        .expect("send");
    assert_eq!(response.status(), 200);
    assert_eq!(response.body(), b"{\"ok\":1}");
    assert_eq!(response.header("retry-after"), Some("2"));
    transport
        .send(&post(
            server.endpoint("/v1/responses"),
            Some(SecretHeader::bearer(key())),
        ))
        .expect("bearer");
    let seen = server.finish();
    assert!(seen[0].head.starts_with("POST /v1/messages HTTP/1.1"));
    assert_eq!(seen[0].header("x-api-key"), Some(KEY));
    assert_eq!(seen[0].header("authorization"), None);
    assert_eq!(seen[0].header("content-type"), Some("application/json"));
    let agent = seen[0].header("user-agent").expect("user-agent");
    assert_eq!(
        agent,
        format!("sc-agent-http/{}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(seen[0].body, br#"{"q":1}"#);
    assert_eq!(
        seen[1].header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
}

#[test]
fn test_error_statuses_are_responses() {
    let server = LoopbackServer::start(vec![
        Reply::status(429, &[("Retry-After", "1.5")], b"busy"),
        Reply::status(404, &[], b"missing"),
        Reply::status(503, &[], b""),
    ]);
    let transport = loopback();
    let mut statuses = Vec::new();
    for path in ["/a", "/b", "/c"] {
        let response = transport
            .send(&post(server.endpoint(path), None))
            .expect("status is a response");
        statuses.push((response.status(), response.into_body()));
    }
    assert_eq!(
        statuses,
        [
            (429, b"busy".to_vec()),
            (404, b"missing".to_vec()),
            (503, Vec::new())
        ]
    );
    assert_eq!(server.finish().len(), 3);
}

#[test]
fn test_redirects_are_not_followed() {
    let server = LoopbackServer::start(vec![Reply::status(
        302,
        &[("Location", "http://invalid.test/")],
        b"",
    )]);
    let response = loopback()
        .send(&post(server.endpoint("/moved"), Some(api_key_header())))
        .expect("302");
    assert_eq!(response.status(), 302);
    assert_eq!(response.header("location"), Some("http://invalid.test/"));
    assert_eq!(server.finish().len(), 1);
}

/// The 8 MiB cap holds while streaming, for success and error statuses.
#[test]
fn test_response_body_cap_is_8_mib_while_streaming() {
    let exact = vec![b'x'; CAP];
    let over = vec![b'x'; CAP + 1];
    let server = LoopbackServer::start(vec![
        Reply::status(200, &[], &exact),
        Reply::status(200, &[], &over),
        Reply::status(500, &[], &over),
    ]);
    let transport = loopback();
    let fits = transport
        .send(&post(server.endpoint("/exact"), None))
        .expect("exactly the cap");
    assert_eq!(fits.body().len(), CAP);
    for path in ["/big", "/big-error"] {
        let error = transport
            .send(&post(server.endpoint(path), None))
            .expect_err("oversize");
        assert_eq!(error.code(), "RESPONSE_INVALID", "{path}");
        assert!(
            error.to_string().contains("exceeds 8388608 bytes"),
            "{error}"
        );
    }
    server.finish();
}

/// One agent per transport: two sends, and a clone's send, share one
/// keep-alive connection. A second connection would never be answered.
#[test]
fn test_agent_is_reused_across_sends() {
    let server = LoopbackServer::start_one_connection(vec![
        Reply::keep_alive(200, &[], b"1"),
        Reply::keep_alive(200, &[], b"2"),
        Reply::keep_alive(200, &[], b"3"),
    ]);
    let transport = loopback();
    let clone = transport.clone();
    for (sender, body) in [(&transport, b"1"), (&transport, b"2"), (&clone, b"3")] {
        let response = sender
            .send(&post(server.endpoint("/pool"), None))
            .expect("pooled send");
        assert_eq!(response.body(), body);
    }
    let seen = server.finish();
    assert!(seen.iter().all(|request| request.connection == 0));
}

#[test]
fn test_timeout_is_network_error_without_the_key() {
    // The server holds the connection until `finish`, so the only clock in
    // play is the client request timeout. 1 s leaves ample time to connect
    // and write the request first, so the stall is the only possible cause.
    let server = LoopbackServer::start(vec![Reply::Stall]);
    let transport = UreqTransport::loopback_for_tests(Timeouts::new(
        Duration::from_secs(2),
        Duration::from_secs(1),
    ));
    let error = send_with_busy_retry(
        &transport,
        &post(server.endpoint("/slow"), Some(api_key_header())),
        &BusyPolicy::default(),
        &CancelToken::new(),
    )
    .expect_err("timeout");
    assert!(
        matches!(
            error,
            JevError::NetworkError {
                kind: NetworkErrorKind::Timeout,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(error.retryable());
    assert_no_key(&error);
    assert_eq!(server.finish().len(), 1, "a timeout is not retried");
}

/// A connect-phase failure returns at once, retryable, sent once. Unix
/// reports the closed loopback port as refused (`Connect`); Windows retries
/// the SYN until the 2 s connect timeout (`Timeout`).
#[test]
fn test_connect_phase_failure_is_retryable_and_not_retried() {
    let endpoint = Endpoint::loopback_http(refused_port(), "/closed");
    let recording = RecordingTransport::new(loopback());
    let error = send_with_busy_retry(
        &recording,
        &post(endpoint, Some(api_key_header())),
        &BusyPolicy::default(),
        &CancelToken::new(),
    )
    .expect_err("refused");
    assert!(
        matches!(
            error,
            JevError::NetworkError {
                kind: NetworkErrorKind::Connect | NetworkErrorKind::Timeout,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(error.retryable());
    assert_no_key(&error);
    assert_eq!(recording.requests().len(), 1, "no connectivity retry");
}

#[test]
fn test_busy_retry_over_loopback_resends_the_same_body() {
    let server = LoopbackServer::start(vec![
        Reply::status(529, &[], b""),
        Reply::status(200, &[], b"done"),
    ]);
    let clock = FakeClock::new();
    let sleeper = RecordingSleeper::new(clock.clone());
    let policy = BusyPolicy::with_test_hooks(clock, sleeper.clone(), Duration::ZERO);
    let response = send_with_busy_retry(
        &loopback(),
        &post(server.endpoint("/retry"), Some(api_key_header())),
        &policy,
        &CancelToken::new(),
    )
    .expect("retried");
    assert_eq!((response.status(), response.body()), (200, &b"done"[..]));
    assert_eq!(sleeper.waits(), [Duration::from_secs(1)]);
    let seen = server.finish();
    assert_eq!(seen[0].body, seen[1].body);
    assert_eq!(seen[1].header("x-api-key"), Some(KEY));
}

/// Moved from the former transports: `Debug` of a request never shows the
/// key, and `SecretHeader` offers only the redacted form and a comparison.
#[test]
fn test_request_debug_and_secret_header_never_show_the_key() {
    let request = post(
        Endpoint::parse("https://x.invalid/").expect("https"),
        Some(api_key_header()),
    );
    let rendered = format!("{request:?}");
    assert!(!rendered.contains(KEY), "{rendered}");
    let secret = request.secret().expect("secret");
    assert_eq!(secret.redacted(), "x-api-key: [redacted]");
    assert!(secret.matches(KEY));
    assert!(!secret.matches("sk-other"));
    assert!(!format!("{secret:?}").contains(KEY));
}
