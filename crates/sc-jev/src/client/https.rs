//! Blocking HTTPS transport for the packaged TypeSafe endpoint.
//!
//! One ureq agent is built per transport in [`HttpsTransport::new`], so
//! retries and later calls reuse its connection pool. The agent refuses
//! cleartext, does not follow redirects, has a connect timeout, and sends a
//! versioned `User-Agent`. Each send applies [`WireRequest::timeout`] as the
//! end-to-end deadline. The response body is capped at
//! [`MAX_RESPONSE_BYTES`]. 4xx, 5xx, and 3xx stay in the
//! [`WireResponse`] so the client can apply the busy-retry policy.

use std::io::Read;
use std::time::Duration;

use sc_agent_types::{JevError, NetworkErrorKind};
use ureq::Agent;

use super::{JevTransport, WireRequest, WireResponse};

/// Largest Jev response body this client will buffer.
///
/// A normal check reply fits well under this. Anything larger is
/// [`JevError::ResponseInvalid`] so a response cannot grow without a bound.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// TCP connect deadline, matching the LLM transport (ADR-SAG-0007).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// `User-Agent` sent on every Jev call.
const USER_AGENT: &str = concat!("sc-jev/", env!("CARGO_PKG_VERSION"));

/// Production transport. Holds no API key; each [`WireRequest`] carries it.
///
/// [`JevTransport::send`] is the only place this type reads the key.
#[derive(Debug, Clone)]
pub struct HttpsTransport {
    agent: Agent,
    require_https: bool,
}

impl HttpsTransport {
    /// Builds a transport that speaks HTTPS only.
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent: build_agent(true),
            require_https: true,
        }
    }

    /// Loopback-only variant for transport tests; allows `http://`.
    #[cfg(test)]
    fn loopback_for_tests() -> Self {
        Self {
            agent: build_agent(false),
            require_https: false,
        }
    }
}

fn build_agent(https_only: bool) -> Agent {
    Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .http_status_as_error(false)
        .https_only(https_only)
        .max_redirects(0)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

impl Default for HttpsTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl JevTransport for HttpsTransport {
    fn send(&self, request: &WireRequest<'_>) -> Result<WireResponse, JevError> {
        if self.require_https && !request.url.starts_with("https://") {
            return Err(JevError::network(
                NetworkErrorKind::Tls,
                "Jev connection failed: URL is not https",
            ));
        }
        #[expect(
            clippy::disallowed_methods,
            reason = "ADR-SAG-0020: the sending transport is the one reader of the key"
        )]
        let authorization = format!("Bearer {}", request.api_key.as_str());
        let mut response = self
            .agent
            .post(request.url)
            .config()
            .timeout_global(Some(request.timeout))
            .build()
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json")
            .send(request.body)
            .map_err(|error| network(ureq_error_kind(&error), &error, &authorization))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut reader = response
            .body_mut()
            .as_reader()
            .take(MAX_RESPONSE_BYTES.saturating_add(1));
        let mut body = Vec::new();
        reader
            .read_to_end(&mut body)
            .map_err(|error| network(io_error_kind(&error), &error, &authorization))?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(JevError::response_invalid(format!(
                "Jev response exceeds {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        Ok(WireResponse {
            status,
            body,
            retry_after,
        })
    }
}

/// `NETWORK_ERROR` for a failed connection or body read.
///
/// The underlying error text is formatted here, so the full header value is
/// removed before the client's key redaction runs.
fn network(kind: NetworkErrorKind, error: &dyn std::fmt::Display, authorization: &str) -> JevError {
    JevError::network(
        kind,
        format!("Jev connection failed: {error}").replace(authorization, "[redacted]"),
    )
}

/// Classifies a ureq failure for [`JevError::retryable`].
fn ureq_error_kind(error: &ureq::Error) -> NetworkErrorKind {
    match error {
        ureq::Error::Timeout(_) => NetworkErrorKind::Timeout,
        ureq::Error::Io(io) => io_error_kind(io),
        _ => NetworkErrorKind::Connect,
    }
}

/// `Timeout` for a timed-out read, otherwise `Connect`.
fn io_error_kind(error: &std::io::Error) -> NetworkErrorKind {
    if error.kind() == std::io::ErrorKind::TimedOut {
        NetworkErrorKind::Timeout
    } else {
        NetworkErrorKind::Connect
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use sc_agent_types::{
        BackendKeyId, CanonicalJudgePayload, JudgeBackend, JudgeRequest, PayloadBundle,
        ResolvedApiKey,
    };

    use super::*;
    use crate::test_server::{LoopbackServer, Reply, UNREACHED_URL, closed_port};
    use crate::{JevClient, JevClientConfig, RetryDelay};

    const KEY: &str = "sk-loopback-secret";

    fn key() -> ResolvedApiKey {
        ResolvedApiKey::try_new(BackendKeyId::TypeSafe, KEY.to_owned()).expect("key")
    }

    fn send(url: &str, timeout: Duration) -> Result<WireResponse, JevError> {
        HttpsTransport::loopback_for_tests().send(&WireRequest {
            url,
            body: br#"{"model":"m"}"#,
            api_key: &key(),
            timeout,
        })
    }

    fn assert_no_key(error: &JevError) {
        let rendered = format!("{error} {error:?} {}", error.fix_hint());
        assert!(!rendered.contains(KEY), "{rendered}");
    }

    #[test]
    fn sends_auth_user_agent_and_body_and_reads_reply() {
        let server = LoopbackServer::start(vec![Reply::status(200, &[], b"{\"ok\":true}")]);
        let response = send(&server.url("/v1/check"), Duration::from_secs(5)).expect("send");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"{\"ok\":true}");
        assert_eq!(response.retry_after, None);
        let seen = server.finish();
        assert_eq!(seen.len(), 1);
        assert!(seen[0].head.starts_with("POST /v1/check HTTP/1.1"));
        assert_eq!(
            seen[0].header("authorization"),
            Some(format!("Bearer {KEY}").as_str())
        );
        assert_eq!(seen[0].header("user-agent"), Some(USER_AGENT));
        assert!(USER_AGENT.starts_with("sc-jev/") && USER_AGENT.len() > "sc-jev/".len());
        assert_eq!(seen[0].header("content-type"), Some("application/json"));
        assert_eq!(seen[0].body, br#"{"model":"m"}"#);
    }

    #[test]
    fn error_statuses_and_retry_after_are_responses() {
        let server = LoopbackServer::start(vec![
            Reply::status(429, &[("Retry-After", "3")], b"slow down"),
            Reply::status(404, &[], b"missing"),
            Reply::status(503, &[], b""),
        ]);
        let busy = send(&server.url("/a"), Duration::from_secs(5)).expect("429");
        assert_eq!((busy.status, busy.retry_after.as_deref()), (429, Some("3")));
        assert_eq!(busy.body, b"slow down");
        let missing = send(&server.url("/b"), Duration::from_secs(5)).expect("404");
        assert_eq!(missing.status, 404);
        let unavailable = send(&server.url("/c"), Duration::from_secs(5)).expect("503");
        assert_eq!(unavailable.status, 503);
        assert!(unavailable.body.is_empty());
        assert_eq!(server.finish().len(), 3);
    }

    #[test]
    fn redirects_are_not_followed() {
        let server = LoopbackServer::start(vec![Reply::status(
            302,
            &[("Location", UNREACHED_URL)],
            b"",
        )]);
        let response = send(&server.url("/moved"), Duration::from_secs(5)).expect("302");
        assert_eq!(response.status, 302);
        assert_eq!(server.finish().len(), 1);
    }

    #[test]
    fn oversize_body_is_refused() {
        let body = vec![b'x'; usize::try_from(MAX_RESPONSE_BYTES).unwrap() + 1];
        let server = LoopbackServer::start(vec![Reply::status(200, &[], &body)]);
        let error = send(&server.url("/big"), Duration::from_secs(30)).expect_err("oversize");
        assert_eq!(error.code(), "RESPONSE_INVALID");
        assert!(error.to_string().contains("exceeds"));
        server.finish();
    }

    #[test]
    fn timeout_and_refused_connection_are_network_errors() {
        // The server holds the connection until `finish` releases it, so the
        // only clock in play is the client timeout under test.
        let server = LoopbackServer::start(vec![Reply::Stall]);
        let error = send(&server.url("/slow"), Duration::from_millis(150)).expect_err("timeout");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(error.to_string().contains("Jev connection failed"));
        assert_no_key(&error);
        server.finish();

        let closed = closed_port();
        let refused = send(&closed.url(), Duration::from_millis(300)).expect_err("unanswered");
        assert_eq!(refused.code(), "NETWORK_ERROR");
        assert_no_key(&refused);
    }

    #[test]
    fn production_transport_refuses_cleartext() {
        let error = HttpsTransport::default()
            .send(&WireRequest {
                url: "http://127.0.0.1:9/x",
                body: b"{}",
                api_key: &key(),
                timeout: Duration::from_secs(1),
            })
            .expect_err("cleartext");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(error.to_string().contains("not https"));
    }

    #[test]
    fn network_detail_drops_the_header_value() {
        let error = network(
            NetworkErrorKind::Connect,
            &format!("echo Bearer {KEY}"),
            &format!("Bearer {KEY}"),
        );
        assert_no_key(&error);
    }

    struct NoWait(Mutex<Vec<Duration>>);

    impl RetryDelay for NoWait {
        fn wait(&self, delay: Duration) {
            self.0.lock().expect("waits").push(delay);
        }
    }

    fn loopback_client(server: &LoopbackServer, attempts: u32) -> (JevClient, Arc<NoWait>) {
        let url = server.url("/v1/check");
        let host = url
            .trim_start_matches("http://")
            .trim_end_matches("/v1/check");
        let config = JevClientConfig::try_new(
            "jev-test",
            host,
            "/v1/check",
            attempts,
            Duration::from_secs(5),
        )
        .expect("config");
        let waits = Arc::new(NoWait(Mutex::new(Vec::new())));
        let client = JevClient::with_config(
            config,
            key(),
            Arc::new(LoopbackClientTransport(HttpsTransport::loopback_for_tests())),
        )
        .with_retry_delay(Arc::clone(&waits) as Arc<dyn RetryDelay>);
        (client, waits)
    }

    /// Rewrites the client's `https://` endpoint to the loopback server.
    struct LoopbackClientTransport(HttpsTransport);

    impl JevTransport for LoopbackClientTransport {
        fn send(&self, request: &WireRequest<'_>) -> Result<WireResponse, JevError> {
            let url = request.url.replacen("https://", "http://", 1);
            self.0.send(&WireRequest {
                url: &url,
                body: request.body,
                api_key: request.api_key,
                timeout: request.timeout,
            })
        }
    }

    fn request() -> JudgeRequest {
        let payload: CanonicalJudgePayload = serde_json::from_value(serde_json::json!({
            "questions": [{"id": "q", "prompt": "p", "choices": ["no", "yes"]}],
            "state": []
        }))
        .expect("payload");
        JudgeRequest::new(
            Arc::new(PayloadBundle::encode(&payload).expect("bundle")),
            None,
        )
    }

    #[test]
    fn client_over_loopback_maps_busy_and_http_errors() {
        let answer = br#"{"model":"jev-test","answers":{"q":{"choice":"yes","confidence":1,"probabilities":{"no":0,"yes":1}}}}"#;
        let server = LoopbackServer::start(vec![
            Reply::status(529, &[("Retry-After", "0.5")], b""),
            Reply::status(200, &[], answer),
            Reply::status(429, &[], b""),
            Reply::status(429, &[], b""),
            Reply::status(500, &[], b"boom"),
        ]);
        let (client, waits) = loopback_client(&server, 2);
        let outcome = client.judge(&request()).expect("judge after busy");
        assert_eq!(outcome.answers()["q"].choice().as_str(), "yes");
        let exhausted = client.judge(&request()).expect_err("busy twice");
        assert_eq!(exhausted.code(), "NETWORK_ERROR");
        assert_eq!(
            exhausted.to_string(),
            "network error: Jev busy: HTTP 429 after 2 sends; retry attempts exhausted"
        );
        let failed = client.judge(&request()).expect_err("500");
        assert_eq!(failed.to_string(), "network error: Jev HTTP 500");
        let waits = waits.0.lock().expect("waits").clone();
        assert_eq!(waits.len(), 2);
        assert!(waits[0] >= Duration::from_millis(500));
        assert!(waits[1] >= Duration::from_secs(1));
        assert_eq!(server.finish().len(), 5);
    }
}
