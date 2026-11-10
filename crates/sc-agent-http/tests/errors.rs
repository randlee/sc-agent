//! The error contract (REQ-SAG-0002): ids, Display, the cause chain, secret
//! redaction and whole provider text.

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Instant;

    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{
        Attempts, BusyReason, Endpoint, HttpClient, HttpError, HttpRequest, HttpResponse,
        LogContext, Method, NetworkKind, Next, ProviderReply, ResendPolicy, SecretHeader, Start,
        Timeouts, Transport, UreqTransport,
    };
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_secrets::{
        ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var,
    };
    use sc_agent_types::{ErrorDetail, ErrorId};

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

    const KEY: &str = "sk-error-contract-secret-77";

    fn reply(status: u16, message: &str) -> ProviderReply {
        ProviderReply::read(
            &HttpResponse::new(status, Vec::new(), message.as_bytes().to_vec()),
            None,
        )
    }

    fn one_of_each() -> Vec<HttpError> {
        vec![
            HttpError::ProviderBusy {
                reason: BusyReason::Sends,
                attempts: 5,
                retry_after_seconds: Some(1.5),
                last: Box::new(reply(429, "busy")),
            },
            HttpError::QuotaExhausted {
                attempts: 1,
                reply: Box::new(reply(429, "quota")),
            },
            HttpError::network(NetworkKind::Timeout, 2, "the request timed out"),
            HttpError::ResponseTooLarge {
                status: 200,
                limit_bytes: 8 * 1024 * 1024,
            },
            Endpoint::parse("http://x.test/").expect_err("http refused"),
            HttpError::invalid_request("the HTTP builder refused a header name or value"),
            HttpError::KeyOnPlainHttp {
                host: "10.0.0.5".to_owned(),
            },
        ]
    }

    /// Sends `request` through `transport` once and returns the error.
    fn send_err(transport: Arc<dyn Transport>, request: &HttpRequest) -> HttpError {
        let memory = MemoryLog::new().expect("memory log");
        let client = HttpClient::new(transport, ResendPolicy::default(), memory.general());
        let Ok(Start::Go(permit)) = client.may_start(request, Attempts::new(), Instant::now())
        else {
            panic!("fresh");
        };
        match client.send_once(request, permit, LogContext::NONE, &Instant::now) {
            Err(error) => error,
            Ok(next) => panic!("expected an error: {next:?}"),
        }
    }

    fn keyed_request() -> HttpRequest {
        HttpRequest::new(
            Method::Post,
            Endpoint::parse("https://api.example.test/v1").expect("https"),
            b"{}".as_slice(),
        )
        .with_secret(SecretHeader::bearer(key()))
        .expect("https")
    }

    fn rendered(error: &HttpError) -> String {
        format!("{error} {error:?} {} {:?}", error.message(), error.detail())
    }

    #[test]
    fn http_error_ids() {
        let errors = one_of_each();
        let ids: HashSet<&str> = errors.iter().map(ErrorId::id).collect();
        assert_eq!(errors.len(), 7);
        assert_eq!(
            ids,
            HashSet::from([
                "http.provider_busy",
                "http.quota_exhausted",
                "http.network",
                "http.response_too_large",
                "http.invalid_endpoint",
                "http.invalid_request",
                "http.key_on_plain_http",
            ])
        );
        for error in &errors {
            assert_eq!(
                error.to_string(),
                format!("{}: {}", error.id(), error.message())
            );
            // detail() is the one assembly of id, message, recovery and cause.
            let mut expected = ErrorDetail::new(error.id(), error.message(), error.cause_detail());
            if let Some(recovery) = error.recovery() {
                expected = expected.with_recovery(recovery);
            }
            assert_eq!(error.detail(), expected);
            assert_eq!(
                std::error::Error::source(error).is_some(),
                error.cause_detail().is_some()
            );
        }
        let with_recovery: HashSet<&str> = errors
            .iter()
            .filter(|error| error.recovery().is_some())
            .map(ErrorId::id)
            .collect();
        assert_eq!(
            with_recovery,
            HashSet::from([
                "http.provider_busy",
                "http.quota_exhausted",
                "http.key_on_plain_http"
            ])
        );
    }

    /// A5-12: the key in a transport error and in a busy reply message is gone
    /// from every error text, Debug output and log event.
    #[test]
    fn key_never_leaks() {
        let echo = HttpError::network(
            NetworkKind::Other,
            1,
            format!("proxy echoed Authorization: Bearer {KEY}"),
        )
        .with_cause(ErrorDetail::new(
            "io.Other",
            format!("echo {KEY}"),
            Some(ErrorDetail::new(
                "foreign.source",
                format!("inner {KEY}"),
                None,
            )),
        ));
        let error = send_err(
            Arc::new(ScriptedTransport::new(vec![Err(echo)])),
            &keyed_request(),
        );
        assert!(!rendered(&error).contains(KEY), "{}", rendered(&error));
        assert!(error.message().contains("Bearer [redacted]"));
        let cause = error.cause_detail().expect("cause kept");
        assert_eq!(cause.message, "echo [redacted]");
        assert_eq!(
            cause.cause.map(|inner| inner.message),
            Some("inner [redacted]".to_owned())
        );

        let busy_body = format!(
            r#"{{"error":{{"type":"overloaded","message":"key {KEY} is busy","code":"{KEY}"}}}}"#
        );
        let memory = MemoryLog::new().expect("memory log");
        let transport = ScriptedTransport::responses(vec![HttpResponse::new(
            529,
            vec![("x-request-id".to_owned(), format!("req-{KEY}"))],
            busy_body.into_bytes(),
        )]);
        let client = HttpClient::new(
            Arc::new(transport),
            ResendPolicy::new(std::num::NonZeroU32::MIN),
            memory.general(),
        );
        let request = keyed_request();
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), Instant::now())
        else {
            panic!("fresh");
        };
        let error = match client.send_once(&request, permit, LogContext::NONE, &Instant::now) {
            Err(error) => error,
            Ok(next) => panic!("{next:?}"),
        };
        let HttpError::ProviderBusy { last, .. } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(last.message, "key [redacted] is busy");
        assert!(!rendered(&error).contains(KEY), "{}", rendered(&error));
        assert!(!format!("{request:?}").contains(KEY));
        let records = memory.records().expect("records");
        assert!(!records.is_empty());
        assert!(!format!("{records:?}").contains(KEY));
    }

    /// REQ-SAG-0002: provider text is redacted, never cut.
    #[test]
    fn long_provider_text_kept() {
        let message = format!("{}{KEY}{}", "a".repeat(50_000), "z".repeat(50_000));
        let body = format!(r#"{{"error":{{"type":"insufficient_quota","message":"{message}"}}}}"#);
        let transport =
            ScriptedTransport::responses(vec![HttpResponse::new(429, vec![], body.into_bytes())]);
        let error = send_err(Arc::new(transport), &keyed_request());
        let HttpError::QuotaExhausted { reply, .. } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(
            reply.message,
            format!("{}[redacted]{}", "a".repeat(50_000), "z".repeat(50_000))
        );
        assert!(error.message().contains(&reply.message));
    }

    /// A5-17: a ureq failure keeps its id and message in the ErrorDetail chain.
    #[test]
    fn lower_layer_cause_kept() {
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            listener.local_addr().expect("addr").port()
        };
        let request = HttpRequest::new(
            Method::Get,
            Endpoint::parse_local(&format!("http://127.0.0.1:{port}/closed")).expect("local"),
            Vec::new(),
        );
        let transport = UreqTransport::new(Timeouts::probe());
        let direct = transport.send(&request).expect_err("closed port");
        let error = send_err(Arc::new(transport), &request);
        assert!(matches!(error, HttpError::Network { .. }), "{error:?}");
        let cause = error.detail().cause.expect("lower-layer cause");
        assert!(
            cause.id.starts_with("io.") || cause.id.starts_with("ureq."),
            "{cause:?}"
        );
        // The cause is the transport's own error, id and message whole (A5-17).
        assert_eq!(Some(cause.clone()), direct.detail().cause);
        if cfg!(unix) {
            assert_eq!(cause.id, "io.ConnectionRefused");
            // The message is the refused connect's own text, unchanged: the same io error,
            // made directly, rendered as ureq renders it.
            let refused =
                std::net::TcpStream::connect(("127.0.0.1", port)).expect_err("the port is closed");
            assert_eq!(cause.message, ureq::Error::Io(refused).to_string());
        }
        // This layer's message does not repeat the cause; the chain carries it.
        assert!(!error.message().contains(&cause.message), "{error}");
        // source() exposes the same cause.
        let source = std::error::Error::source(&error).expect("source");
        assert_eq!(
            source.to_string(),
            format!("{}: {}", cause.id, cause.message)
        );
        // The client adds only the send count; the transport's cause is unchanged.
        assert_eq!(direct.cause_detail(), error.cause_detail());
    }

    /// The returned value is the redacted error, also for a refused resend.
    #[test]
    fn refused_resend_is_redacted() {
        let echo = HttpError::network(
            NetworkKind::Timeout,
            1,
            format!("ureq.Timeout.Global: {KEY}"),
        );
        let transport = ScriptedTransport::new(vec![Err(echo)]);
        let memory = MemoryLog::new().expect("memory log");
        let client = HttpClient::new(
            Arc::new(transport),
            ResendPolicy::default(),
            memory.general(),
        );
        let request = keyed_request();
        let now = Instant::now();
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), now) else {
            panic!("fresh");
        };
        let Ok(Next::ResendAt { pending, .. }) =
            client.send_once(&request, permit, LogContext::NONE, &|| now)
        else {
            panic!("timeout is held for resend");
        };
        let error = client
            .may_resend(&request, pending, now + std::time::Duration::from_secs(90))
            .expect_err("budget");
        assert!(matches!(
            error,
            HttpError::Network {
                kind: NetworkKind::Timeout,
                attempts: 1,
                ..
            }
        ));
        assert!(!rendered(&error).contains(KEY));
    }
}
