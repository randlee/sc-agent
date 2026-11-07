//! Busy retry under the decided numbers (owner decision (b), REQ-SAG-0505
//! amendment, ADR-SAG-0007 r-3 amendment).
//!
//! Needs `--features test-util`: every test drives
//! `BusyPolicy::with_test_hooks` (fake clock, recording sleeper,
//! deterministic jitter), so nothing sleeps.

use std::sync::Arc;
use std::time::Duration;

use sc_agent_http::test_util::{FakeClock, RecordingSleeper, ScriptedTransport};
use sc_agent_http::{
    BusyPolicy, Endpoint, HttpClient, HttpRequest, HttpResponse, Method, SecretHeader,
    send_and_parse, send_with_busy_retry,
};
use sc_agent_types::{BackendKeyId, CancelToken, JevError, NetworkErrorKind, ResolvedApiKey};

const KEY: &str = "sk-retry-secret";

fn secs(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds)
}

fn request() -> HttpRequest {
    let key = ResolvedApiKey::try_new(BackendKeyId::OpenAi, KEY.to_owned()).expect("key");
    HttpRequest::new(
        Method::Post,
        Endpoint::parse("https://api.example.test/v1/x").expect("https"),
        Arc::from(&b"{\"same\":\"body\"}"[..]),
    )
    .with_secret(SecretHeader::bearer(key))
}

fn busy(status: u16, retry_after: Option<&str>) -> HttpResponse {
    let headers = retry_after
        .map(|value| vec![("Retry-After".to_owned(), value.to_owned())])
        .unwrap_or_default();
    HttpResponse::new(status, headers, Vec::new())
}

fn ok() -> HttpResponse {
    HttpResponse::new(200, Vec::new(), b"{}".to_vec())
}

/// Policy, sleeper and clock with a fixed `jitter`.
struct Hooks {
    clock: FakeClock,
    sleeper: RecordingSleeper,
    policy: BusyPolicy,
}

fn fake(jitter: Duration) -> Hooks {
    let clock = FakeClock::new();
    let sleeper = RecordingSleeper::new(clock.clone());
    let policy = BusyPolicy::with_test_hooks(clock.clone(), sleeper.clone(), jitter);
    Hooks {
        clock,
        sleeper,
        policy,
    }
}

fn run(transport: &ScriptedTransport, policy: &BusyPolicy) -> Result<HttpResponse, JevError> {
    send_with_busy_retry(transport, &request(), policy, &CancelToken::new())
}

fn network_kind(error: &JevError) -> NetworkErrorKind {
    match error {
        JevError::NetworkError { kind, .. } => *kind,
        other => panic!("expected NETWORK_ERROR, got {other:?}"),
    }
}

fn detail(error: &JevError) -> &str {
    match error {
        JevError::NetworkError { detail, .. } => detail,
        other => panic!("expected NETWORK_ERROR, got {other:?}"),
    }
}

/// `BusyPolicy::default()`: 429/502/529 only, 4 sends, three waits of
/// 1, 2, 4 s, each plus the jitter (here the 250 ms maximum).
#[test]
fn test_busy_policy_numbers() {
    assert_eq!(BusyPolicy::default().max_sends(), 4);
    let jitter = Duration::from_millis(250);
    for status in [429_u16, 502, 529] {
        let hooks = fake(jitter);
        let transport = ScriptedTransport::responses(vec![busy(status, None); 5]);
        let error = run(&transport, &hooks.policy).expect_err("exhausted");
        assert_eq!(transport.sends(), 4, "status {status}");
        assert_eq!(
            hooks.sleeper.waits(),
            [secs(1.25), secs(2.25), secs(4.25)],
            "status {status}"
        );
        assert_eq!(network_kind(&error), NetworkErrorKind::Busy);
        assert_eq!(
            detail(&error),
            format!("HTTP {status} busy; 4 sends exhausted")
        );
        let bodies: Vec<Vec<u8>> = transport
            .requests()
            .iter()
            .map(|sent| sent.body().to_vec())
            .collect();
        assert!(bodies.windows(2).all(|pair| pair[0] == pair[1]));
    }
    // Every other status is returned as is, after one send.
    for status in [200_u16, 400, 404, 500, 503, 504] {
        let hooks = fake(jitter);
        let transport = ScriptedTransport::responses(vec![busy(status, Some("1")), ok()]);
        let response = run(&transport, &hooks.policy).expect("not busy");
        assert_eq!(response.status(), status);
        assert_eq!(transport.sends(), 1);
        assert!(hooks.sleeper.waits().is_empty());
    }
}

/// Absent and malformed `Retry-After` are treated alike: 1, 2, 4 s with the
/// default sends; 8, 16 s and the 30 s cap only with more sends.
#[test]
fn test_missing_and_malformed_retry_after_back_off_exponentially() {
    let hooks = fake(Duration::ZERO);
    let transport = ScriptedTransport::responses(vec![
        busy(429, None),
        busy(502, Some("soon")),
        busy(529, Some("Wed, 21 Oct 2026 07:28:00 GMT")),
        ok(),
    ]);
    run(&transport, &hooks.policy).expect("fourth send succeeds");
    assert_eq!(hooks.sleeper.waits(), [secs(1.0), secs(2.0), secs(4.0)]);

    let hooks = fake(Duration::ZERO);
    let policy = hooks.policy.clone().with_max_sends(7);
    let transport = ScriptedTransport::responses(vec![busy(429, Some("-1")); 7]);
    let error = run(&transport, &policy).expect_err("exhausted");
    assert_eq!(
        hooks.sleeper.waits(),
        [1.0, 2.0, 4.0, 8.0, 16.0, 30.0].map(secs)
    );
    assert_eq!(detail(&error), "HTTP 429 busy; 7 sends exhausted");
    assert_eq!(hooks.clock.now(), secs(61.0));
}

/// The `Retry-After` grammar table (owner decision (b)).
#[test]
fn test_retry_after_grammar() {
    let nines_32 = "9".repeat(32);
    let digits_33 = "1".repeat(33);
    let table: Vec<(&str, Duration)> = vec![
        // Valid: honoured, clamped to 0..=30 s.
        ("3", secs(3.0)),
        ("2.5", secs(2.5)),
        ("0", Duration::ZERO),
        ("45", secs(30.0)),
        (&nines_32, secs(30.0)),
        // Malformed: the first backoff wait, 1 s.
        ("-1", secs(1.0)),
        ("1e9", secs(1.0)),
        ("inf", secs(1.0)),
        ("NaN", secs(1.0)),
        ("", secs(1.0)),
        ("Wed, 21 Oct 2026 07:28:00 GMT", secs(1.0)),
        (&digits_33, secs(1.0)),
    ];
    for (header, expected) in table {
        let hooks = fake(Duration::ZERO);
        let transport = ScriptedTransport::responses(vec![busy(429, Some(header)), ok()]);
        run(&transport, &hooks.policy).expect("second send succeeds");
        assert_eq!(hooks.sleeper.waits(), [expected], "{header:?}");
    }
    // A valid zero wait does not loop: the 4-send cap still bounds it.
    let hooks = fake(Duration::ZERO);
    let transport = ScriptedTransport::responses(vec![busy(429, Some("0")); 6]);
    let error = run(&transport, &hooks.policy).expect_err("exhausted");
    assert_eq!(transport.sends(), 4);
    assert_eq!(hooks.sleeper.waits(), [Duration::ZERO; 3]);
    assert_eq!(detail(&error), "HTTP 429 busy; 4 sends exhausted");
}

/// Both exhaustion reasons are `Busy` and retryable, told apart by detail.
#[test]
fn test_exhaustion_reason_distinguishable() {
    let hooks = fake(Duration::ZERO);
    let transport = ScriptedTransport::responses(vec![busy(429, Some("1")); 4]);
    let sends = run(&transport, &hooks.policy).expect_err("sends");
    assert_eq!(transport.sends(), 4);

    let hooks = fake(Duration::ZERO);
    // Each send takes 45 s: after the second busy reply (t = 120 s) no
    // budget is left, so a third attempt never starts.
    let transport = ScriptedTransport::responses(vec![busy(429, Some("30")); 4])
        .with_send_duration(hooks.clock.clone(), secs(45.0));
    let budget = run(&transport, &hooks.policy).expect_err("budget");
    assert_eq!(transport.sends(), 2);
    assert_eq!(hooks.sleeper.waits(), [secs(30.0)]);

    for error in [&sends, &budget] {
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert_eq!(network_kind(error), NetworkErrorKind::Busy);
        assert!(error.retryable());
    }
    assert!(detail(&sends).contains("4 sends exhausted"), "{sends}");
    assert!(!detail(&sends).contains("budget"), "{sends}");
    assert!(
        detail(&budget).contains("retry budget exhausted"),
        "{budget}"
    );
    assert!(!detail(&budget).contains("sends exhausted"), "{budget}");
    assert_eq!(
        detail(&budget),
        "HTTP 429 busy after 2 sends; retry budget exhausted"
    );
}

/// Every wait is `min(wait + jitter, budget remaining)`.
#[test]
fn test_retry_sleep_capped_by_budget() {
    let hooks = fake(Duration::from_millis(200));
    // The first send takes 80 s; Retry-After 30 s is cut to the 10 s left.
    let transport = ScriptedTransport::responses(vec![busy(529, Some("30")), ok()])
        .with_send_duration(hooks.clock.clone(), secs(80.0));
    let response = run(&transport, &hooks.policy).expect("the capped retry runs");
    assert_eq!(response.status(), 200);
    assert_eq!(hooks.sleeper.waits(), [secs(10.0)]);
    assert_eq!(transport.sends(), 2);

    // Below the cap the jitter is added in full.
    let hooks = fake(Duration::from_millis(200));
    let transport = ScriptedTransport::responses(vec![busy(529, Some("30")), ok()])
        .with_send_duration(hooks.clock.clone(), secs(10.0));
    run(&transport, &hooks.policy).expect("retry");
    assert_eq!(hooks.sleeper.waits(), [secs(30.2)]);
}

/// A cancel during a wait returns `Cancelled` within one 100 ms slice; a
/// cancel before the first attempt sends nothing.
#[test]
fn test_cancel_interrupts_sleep() {
    let cancel = CancelToken::new();
    let clock = FakeClock::new();
    let sleeper = RecordingSleeper::new(clock.clone())
        .with_cancel_after(cancel.clone(), Duration::from_millis(1_050));
    let policy = BusyPolicy::with_test_hooks(clock, sleeper.clone(), Duration::ZERO);
    let transport = ScriptedTransport::responses(vec![busy(429, Some("30")), ok()]);
    let error =
        send_with_busy_retry(&transport, &request(), &policy, &cancel).expect_err("cancelled");
    assert_eq!(network_kind(&error), NetworkErrorKind::Cancelled);
    assert!(!error.retryable());
    assert_eq!(transport.sends(), 1);
    assert_eq!(sleeper.waits(), [secs(30.0)]);
    assert!(
        sleeper
            .slices()
            .iter()
            .all(|slice| *slice <= Duration::from_millis(100))
    );
    let slept = sleeper.slept();
    assert!(slept >= Duration::from_millis(1_050), "{slept:?}");
    assert!(slept <= Duration::from_millis(1_150), "{slept:?}");

    let cancelled = CancelToken::new();
    cancelled.cancel();
    let transport = ScriptedTransport::responses(vec![ok()]);
    let error = send_with_busy_retry(&transport, &request(), &BusyPolicy::default(), &cancelled)
        .expect_err("cancelled before send");
    assert_eq!(network_kind(&error), NetworkErrorKind::Cancelled);
    assert_eq!(transport.sends(), 0);
}

/// A transport error or timeout after the body was sent is never retried.
#[test]
fn test_no_retry_after_body_sent() {
    let failures = [
        JevError::network(
            NetworkErrorKind::Timeout,
            "timed out reading the response after the body was sent",
        ),
        JevError::network(NetworkErrorKind::Connect, "connection reset after send"),
        JevError::response_invalid("HTTP 200 response exceeds 8388608 bytes"),
    ];
    for failure in failures {
        let hooks = fake(Duration::ZERO);
        let transport = ScriptedTransport::new(vec![Err(failure.clone()), Ok(ok())]);
        let error = run(&transport, &hooks.policy).expect_err("not retried");
        assert_eq!(error, failure);
        assert_eq!(transport.sends(), 1);
        assert!(hooks.sleeper.waits().is_empty());
    }
}

/// Connect and DNS failures return at once: `Connect`, retryable, no wait,
/// no connectivity probing.
#[test]
fn test_connect_failure_returns_immediately_retryable() {
    for detail_text in [
        "connection failed: Connection refused",
        "connection failed: host not found",
    ] {
        let hooks = fake(Duration::ZERO);
        let transport = ScriptedTransport::new(vec![
            Err(JevError::network(NetworkErrorKind::Connect, detail_text)),
            Ok(ok()),
        ]);
        let error = run(&transport, &hooks.policy).expect_err("connect");
        assert_eq!(network_kind(&error), NetworkErrorKind::Connect);
        assert!(error.retryable());
        assert_eq!(detail(&error), detail_text);
        assert_eq!(transport.sends(), 1);
        assert!(hooks.sleeper.waits().is_empty());
        assert_eq!(hooks.clock.now(), Duration::ZERO);
    }
}

/// Redaction runs once, in `send_with_busy_retry`, on errors any transport
/// returns, whatever the variant.
#[test]
fn test_transport_error_with_secret_is_redacted() {
    let leaks = [
        JevError::network(
            NetworkErrorKind::Connect,
            format!("echo Authorization: Bearer {KEY}"),
        ),
        JevError::response_invalid(format!("body echoed {KEY}")),
        JevError::input_invalid(format!("{KEY} in detail")),
    ];
    for leak in leaks {
        let hooks = fake(Duration::ZERO);
        let transport = ScriptedTransport::new(vec![Err(leak.clone())]);
        let error = run(&transport, &hooks.policy).expect_err("leak");
        assert_eq!(error.code(), leak.code());
        let rendered = format!("{error} {error:?} {}", error.fix_hint());
        assert!(!rendered.contains(KEY), "{rendered}");
        assert!(rendered.contains("[redacted]"), "{rendered}");
    }
    // The scripted transport saw the secret only as a header it can compare.
    let transport = ScriptedTransport::responses(vec![ok()]);
    run(&transport, &fake(Duration::ZERO).policy).expect("ok");
    let sent = transport.requests();
    let secret = sent[0].secret().expect("secret");
    assert!(secret.matches(KEY));
    assert_eq!(secret.redacted(), "Authorization: Bearer [redacted]");
}

/// `send_and_parse` redacts the parse closure's error as well, and retries
/// busy replies before the closure runs (RBP-F007).
#[test]
fn test_send_and_parse_redacts_parse_errors() {
    let hooks = fake(Duration::ZERO);
    let transport = ScriptedTransport::responses(vec![
        busy(429, None),
        HttpResponse::new(200, Vec::new(), format!("echo {KEY}").into_bytes()),
    ]);
    let error = send_and_parse(
        &transport,
        &request(),
        &hooks.policy,
        &CancelToken::new(),
        |response| -> Result<(), JevError> {
            Err(JevError::response_invalid(
                String::from_utf8_lossy(response.body()).into_owned(),
            ))
        },
    )
    .expect_err("parse error");
    assert_eq!(error.to_string(), "response is invalid: echo [redacted]");
    assert_eq!(transport.sends(), 2);
    assert_eq!(hooks.sleeper.waits(), [secs(1.0)]);

    let client = HttpClient::new(ScriptedTransport::responses(vec![ok()]))
        .with_busy_policy(fake(Duration::ZERO).policy);
    let status = client
        .send_and_parse(&request(), |response| Ok(response.status()))
        .expect("ok");
    assert_eq!(status, 200);
    assert_eq!(client.transport().sends(), 1);
}
