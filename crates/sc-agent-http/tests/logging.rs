//! REQ-SAG-0008 / ANC-007: every http.* event goes to the injected
//! GeneralLogger with the caller's LogContext, without key or body; a log
//! failure never changes a result (A5-14, A5-15, A5-20).

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::num::NonZeroU32;
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    use sc_agent_http::testing::{BlockingTransport, ScriptedTransport};
    use sc_agent_http::{
        Attempts, Endpoint, HttpClient, HttpError, HttpRequest, HttpResponse, LogContext, Method,
        NetworkKind, Next, ResendPolicy, SecretHeader, Start, Transport,
    };
    use sc_agent_log::GeneralLogger;
    use sc_agent_log::testing::{MemoryLog, MemoryRecord};
    use sc_agent_secrets::{
        ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var,
    };
    use sc_agent_types::SessionId;
    use serde_json::Value;

    struct OneVar;
    impl EnvSource for OneVar {
        fn var(&self, name: &str) -> Option<String> {
            (name == "HTTP_TEST_KEY").then(|| KEY.to_owned())
        }
    }
    struct NoKeyring;
    impl KeyringSource for NoKeyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            Ok(None)
        }
    }

    fn key() -> ApiKey {
        let sources = KeySources {
            env: &OneVar,
            repo_root: None,
            keyring: &NoKeyring,
        };
        resolve_var(&"HTTP_TEST_KEY".parse().expect("var"), &sources)
            .expect("key")
            .key
    }

    const KEY: &str = "sk-logging-secret-31415";
    /// Marker in every body sent or received: it must never reach a log.
    const BODY: &str = "BODY-MARKER-271828";

    fn request() -> HttpRequest {
        HttpRequest::new(
            Method::Post,
            Endpoint::parse("https://user:pw@api.example.test:8443/v1/x?token=q").expect("https"),
            format!(r#"{{"text":"{BODY}"}}"#).into_bytes(),
        )
        .with_header("x-trace", "HEADER-VALUE-1618")
        .expect("not a credential header")
        .with_secret(SecretHeader::bearer(key()))
        .expect("https")
    }

    fn response(status: u16, body: &str) -> HttpResponse {
        HttpResponse::new(
            status,
            vec![
                ("x-echo".to_owned(), format!("{KEY} {BODY}")),
                ("request-id".to_owned(), format!("req-{KEY}-7")),
            ],
            body.replace("MARK", &format!("{BODY} {KEY}")).into_bytes(),
        )
    }

    fn timeout() -> HttpError {
        HttpError::network(
            NetworkKind::Timeout,
            1,
            format!("ureq.Timeout.Global: {KEY}"),
        )
    }

    fn connect() -> HttpError {
        HttpError::network(
            NetworkKind::Connect,
            1,
            format!("io.ConnectionRefused: refused {KEY}"),
        )
    }

    /// One admitted send at `now`.
    fn send(
        client: &HttpClient,
        request: &HttpRequest,
        ctx: LogContext,
        now: Instant,
    ) -> Result<Next, HttpError> {
        let Ok(Start::Go(permit)) = client.may_start(request, Attempts::new(), now) else {
            panic!("fresh");
        };
        client.send_once(request, permit, ctx, &|| now)
    }

    fn actions(records: &[MemoryRecord]) -> Vec<&str> {
        records
            .iter()
            .map(|record| record.action.as_str())
            .collect()
    }

    fn find<'r>(records: &'r [MemoryRecord], action: &str) -> &'r MemoryRecord {
        records
            .iter()
            .find(|record| record.action == action)
            .unwrap_or_else(|| panic!("no {action} in {:?}", actions(records)))
    }

    fn policy(max: u32) -> ResendPolicy {
        ResendPolicy::new(NonZeroU32::new(max).expect("nonzero"))
    }

    /// Runs a busy, a busy-exhausted, a budget-exhausted, a quota, a timeout and
    /// a connect-failure send on `log`, and returns their results.
    fn scenarios(log: &GeneralLogger, ctx: LogContext) -> Vec<String> {
        let request = request();
        let now = Instant::now();
        let mut results = Vec::new();
        let mut run = |replies: Vec<Result<HttpResponse, HttpError>>, max: u32| {
            let client = HttpClient::new(
                Arc::new(ScriptedTransport::new(replies)),
                policy(max),
                log.clone(),
            );
            results.push(format!("{:?}", send(&client, &request, ctx, now)));
            client
        };
        // busy, held for resend
        run(vec![Ok(response(429, "busy MARK"))], 5);
        // busy on the last allowed send
        run(vec![Ok(response(529, "busy MARK"))], 1);
        // quota
        run(
            vec![Ok(response(
                429,
                r#"{"error":{"code":"insufficient_quota","message":"MARK"}}"#,
            ))],
            5,
        );
        // timeout, resent
        run(vec![Err(timeout())], 5);
        // connect failure
        run(vec![Err(connect())], 5);
        // budget exhausted: the first send started 90 s before the busy reply
        let client = HttpClient::new(
            Arc::new(ScriptedTransport::new(vec![Ok(response(502, "busy MARK"))])),
            policy(5),
            log.clone(),
        );
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), now) else {
            panic!("fresh");
        };
        let clock = std::cell::Cell::new(now);
        let late = || {
            let read = clock.get();
            clock.set(now + std::time::Duration::from_secs(90));
            read
        };
        results.push(format!(
            "{:?}",
            client.send_once(&request, permit, ctx, &late)
        ));
        results
    }

    #[test]
    fn events_logged() {
        let memory = MemoryLog::new().expect("memory log");
        let session = SessionId::new();
        let ctx = LogContext::request(session, Some(7));
        let results = scenarios(&memory.general(), ctx);
        let records = memory.records().expect("records");
        for action in [
            "http.send",
            "http.response",
            "http.busy",
            "http.busy_exhausted",
            "http.budget_exhausted",
            "http.quota_exhausted",
            "http.timeout_resend",
            "http.transport_failed",
        ] {
            let record = find(&records, action);
            assert_eq!(record.target, "sc_agent_http");
            assert_eq!(
                record.fields.get("session_id").and_then(Value::as_str),
                Some(session.to_string().as_str()),
                "{action}"
            );
            assert_eq!(
                record.fields.get("position"),
                Some(&Value::from(7)),
                "{action}"
            );
        }
        assert!(
            records
                .iter()
                .all(|record| record.action.starts_with("http."))
        );
        let send = find(&records, "http.send");
        assert_eq!(send.fields.get("method"), Some(&Value::from("POST")));
        assert_eq!(
            send.fields.get("endpoint"),
            Some(&Value::from("https://api.example.test/v1/x"))
        );
        assert_eq!(send.fields.get("max_sends"), Some(&Value::from(5)));
        for action in ["http.response", "http.transport_failed"] {
            assert!(
                find(&records, action)
                    .fields
                    .get("elapsed_ms")
                    .is_some_and(Value::is_u64),
                "{action}"
            );
        }
        assert_eq!(
            find(&records, "http.transport_failed").fields.get("error"),
            Some(&Value::from("http.network"))
        );
        // RSH-007: the provider request id, redacted, and the host.
        for action in ["http.response", "http.busy", "http.quota_exhausted"] {
            assert_eq!(
                find(&records, action).fields.get("provider_request_id"),
                Some(&Value::from("req-[redacted]-7")),
                "{action}"
            );
        }
        for action in ["http.busy", "http.busy_exhausted", "http.budget_exhausted"] {
            let record = find(&records, action);
            assert_eq!(
                record.fields.get("host"),
                Some(&Value::from("api.example.test")),
                "{action}"
            );
            assert_eq!(
                record.fields.get("endpoint"),
                Some(&Value::from("https://api.example.test/v1/x")),
                "{action}"
            );
        }
        let busy = find(&records, "http.busy");
        for field in [
            "status",
            "attempt",
            "holdoff_ms",
            "retry_after_honoured",
            "host_paused_until_ms",
        ] {
            assert!(busy.fields.contains_key(field), "http.busy lacks {field}");
        }
        // No body, header value, URL secret or key in any record.
        let text = format!("{records:?}");
        for hidden in [KEY, BODY, "HEADER-VALUE-1618", "token=q", "pw"] {
            assert!(!text.contains(hidden), "{hidden} reached the log");
        }
        // The session id is the event's correlation id.
        let events = memory.events().expect("events");
        assert!(events.iter().all(|event| {
            event
                .correlation_id
                .as_ref()
                .map(|id| id.as_str().to_owned())
                == Some(session.to_string())
        }));
        assert_eq!(results.len(), 6);
    }

    /// A log failure never changes a result.
    #[test]
    fn failing_logger_leaves_results_unchanged() {
        let healthy = MemoryLog::new().expect("memory log");
        let broken = MemoryLog::new().expect("memory log");
        broken.logger().shutdown().expect("shutdown");
        let failing = GeneralLogger::injected(broken.logger());
        assert!(
            failing
                .write(
                    sc_agent_log::Level::Info,
                    "sc_agent_http",
                    "http.send",
                    LogContext::NONE,
                    serde_json::Map::new()
                )
                .is_err(),
            "the logger must really fail for this test"
        );
        let ctx = LogContext::request(SessionId::new(), None);
        let with_log = scenarios(&healthy.general(), ctx);
        let without = scenarios(&failing, ctx);
        let strip = |results: Vec<String>| -> Vec<String> {
            // ResendAt instants and jitter differ run to run; compare the shape.
            results
                .into_iter()
                .map(|text| text.split(" at:").next().unwrap_or_default().to_owned())
                .collect()
        };
        assert_eq!(strip(with_log), strip(without));
    }

    /// A5-20: two concurrent requests on one client log their own session ids.
    #[test]
    fn concurrent_busy_carry_session_ids() {
        let memory = MemoryLog::new().expect("memory log");
        let gate = BlockingTransport::new(Ok(HttpResponse::new(429, vec![], b"{}".to_vec())));
        let client = HttpClient::new(
            Arc::new(gate.clone()) as Arc<dyn Transport>,
            ResendPolicy::default(),
            memory.general(),
        );
        let sessions = [SessionId::new(), SessionId::new()];
        let now = Instant::now();
        let workers: Vec<_> = sessions
            .iter()
            .enumerate()
            .map(|(position, session)| {
                let client = client.clone();
                let ctx = LogContext::request(*session, Some(position as u64));
                let request = request();
                // Both are admitted before either busy reply pauses the host.
                let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), now) else {
                    panic!("fresh");
                };
                thread::spawn(move || {
                    matches!(
                        client.send_once(&request, permit, ctx, &|| now),
                        Ok(Next::ResendAt { .. })
                    )
                })
            })
            .collect();
        // Both sends are in flight at once before either may return.
        gate.wait_started(2);
        gate.release();
        for worker in workers {
            assert!(
                worker.join().expect("worker"),
                "each send is held for resend"
            );
        }
        let records = memory.records().expect("records");
        let busy: Vec<(String, Value)> = records
            .iter()
            .filter(|record| record.action == "http.busy")
            .map(|record| {
                (
                    record
                        .fields
                        .get("session_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    record
                        .fields
                        .get("position")
                        .cloned()
                        .unwrap_or(Value::Null),
                )
            })
            .collect();
        assert_eq!(busy.len(), 2);
        for (position, session) in sessions.iter().enumerate() {
            assert!(
                busy.contains(&(session.to_string(), Value::from(position as u64))),
                "{session} at {position}: {busy:?}"
            );
        }
        assert_ne!(busy[0].0, busy[1].0);
    }
}
