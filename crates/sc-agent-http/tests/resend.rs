//! REQ-SAG-0601 / REQ-SAG-0602 / REQ-SAG-0012 acceptance over a scripted
//! transport and an explicit clock. No sleep, no network.

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::num::NonZeroU32;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use sc_agent_http::testing::{ManualClock, ScriptedTransport};
    use sc_agent_http::{
        Attempts, BusyReason, Endpoint, HttpClient, HttpError, HttpRequest, HttpResponse,
        LogContext, Method, NetworkKind, Next, ResendPolicy, SecretHeader, Start,
    };
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_secrets::{
        ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var,
    };

    const KEY: &str = "sk-resend-secret-0123456789";

    struct OneVar;
    impl EnvSource for OneVar {
        fn var(&self, name: &str) -> Option<String> {
            (name == "RESEND_TEST_KEY").then(|| KEY.to_owned())
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
        resolve_var(&"RESEND_TEST_KEY".parse().expect("var"), &sources)
            .expect("key")
            .key
    }

    fn secs(seconds: f64) -> Duration {
        Duration::from_secs_f64(seconds)
    }

    fn request_to(url: &str) -> HttpRequest {
        HttpRequest::new(
            Method::Post,
            Endpoint::parse(url).expect("https"),
            b"{\"same\":\"body\"}".as_slice(),
        )
        .with_header("content-type", "application/json")
        .expect("not a credential header")
        .with_secret(SecretHeader::bearer(key()))
        .expect("https carries a key")
    }

    fn request() -> HttpRequest {
        request_to("https://api.example.test/v1/x")
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

    fn timeout() -> HttpError {
        HttpError::network(
            NetworkKind::Timeout,
            1,
            "ureq.Timeout.Global: timeout: global".to_owned(),
        )
    }

    fn policy(max_sends: u32) -> ResendPolicy {
        ResendPolicy::new(NonZeroU32::new(max_sends).expect("nonzero"))
    }

    fn client(transport: &ScriptedTransport, policy: ResendPolicy) -> HttpClient {
        let memory = MemoryLog::new().expect("memory log");
        HttpClient::new(Arc::new(transport.clone()), policy, memory.general())
    }

    fn advance_to(clock: &ManualClock, at: Instant) {
        let now = clock.now();
        if at > now {
            clock.advance(at - now);
        }
    }

    /// A queue of one: admits, sends, holds off on the manual clock and resends
    /// until the request ends. Returns the outcome and every ResendAt holdoff.
    fn drive(
        client: &HttpClient,
        request: &HttpRequest,
        clock: &ManualClock,
    ) -> (Result<(HttpResponse, Attempts), HttpError>, Vec<Duration>) {
        let now = || clock.now();
        let mut holdoffs = Vec::new();
        let mut start = client
            .may_start(request, Attempts::new(), clock.now())
            .expect("a fresh request may start");
        loop {
            let permit = match start {
                Start::Go(permit) => permit,
                Start::Wait { at, state } => {
                    advance_to(clock, at);
                    start = client
                        .may_start(request, state, clock.now())
                        .expect("may start after the pause");
                    continue;
                }
            };
            match client.send_once(request, permit, LogContext::NONE, &now) {
                Ok(Next::Done { response, attempts }) => {
                    return (Ok((response, attempts)), holdoffs);
                }
                Err(error) => return (Err(error), holdoffs),
                Ok(Next::ResendAt { at, mut pending }) => {
                    holdoffs.push(at.saturating_duration_since(clock.now()));
                    advance_to(clock, at);
                    loop {
                        match client.may_resend(request, pending, clock.now()) {
                            Err(error) => return (Err(error), holdoffs),
                            Ok(Start::Go(permit)) => {
                                start = Start::Go(permit);
                                break;
                            }
                            Ok(Start::Wait { at, state }) => {
                                advance_to(clock, at);
                                pending = state;
                            }
                        }
                    }
                }
            }
        }
    }

    /// The holdoff `send_once` returns for one busy `response` at a fixed clock.
    fn first_holdoff(response: HttpResponse) -> Duration {
        let transport = ScriptedTransport::responses(vec![response]);
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let request = request();
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), clock.now()) else {
            panic!("fresh request");
        };
        match client.send_once(&request, permit, LogContext::NONE, &|| clock.now()) {
            Ok(Next::ResendAt { at, .. }) => at - clock.now(),
            other => panic!("expected ResendAt: {other:?}"),
        }
    }

    fn assert_between(holdoff: Duration, low: f64, high: f64) {
        assert!(
            holdoff >= secs(low) && holdoff <= secs(high),
            "{holdoff:?} not in {low}..={high} s"
        );
    }

    // A5-02
    #[test]
    fn busy_every_time_default() {
        let replies = [429, 502, 529, 429, 502, 529]
            .map(|status| busy(status, None))
            .to_vec();
        let transport = ScriptedTransport::responses(replies);
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let (result, holdoffs) = drive(&client, &request(), &clock);
        match result {
            Err(HttpError::ProviderBusy {
                reason: BusyReason::Sends,
                attempts: 5,
                last,
                ..
            }) => assert_eq!(last.status, 502, "the fifth reply"),
            other => panic!("expected provider_busy (sends, 5): {other:?}"),
        }
        assert_eq!(transport.sends(), 5);
        assert_eq!(holdoffs.len(), 4);
    }

    // A5-02: no hard cap; max_sends = 2 sends twice, 7 sends seven times.
    #[test]
    fn busy_every_time_max_two() {
        for max in [2_u32, 7] {
            let transport =
                ScriptedTransport::responses((0..10).map(|_| busy(529, None)).collect());
            let client = client(&transport, policy(max));
            let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
            assert!(
                matches!(result, Err(HttpError::ProviderBusy { reason: BusyReason::Sends, attempts, .. }) if attempts == max),
                "{max}: {result:?}"
            );
            assert_eq!(transport.sends(), max as usize);
        }
    }

    // A5-03
    #[test]
    fn retry_after_jittered() {
        for _ in 0..16 {
            assert_between(first_holdoff(busy(429, Some("3"))), 3.0, 3.25);
        }
        assert_between(first_holdoff(busy(429, Some("2.5"))), 2.5, 2.75);
    }

    // A5-03
    #[test]
    fn retry_after_capped() {
        for value in ["99", "30", &"9".repeat(32)] {
            assert_between(first_holdoff(busy(502, Some(value))), 30.0, 30.25);
        }
    }

    // A5-03: no Retry-After: 0 storm (REQ-SAG-0012).
    #[test]
    fn retry_after_zero_floored() {
        for value in ["0", "0.1", "0.0"] {
            assert_between(first_holdoff(busy(429, Some(value))), 0.25, 0.5);
        }
    }

    // A5-03: malformed or absent Retry-After falls back to 1, 2, 4, 8 s (+ jitter).
    #[test]
    fn retry_after_invalid_falls_back() {
        for header in [
            Some("abc"),
            None,
            Some("-1"),
            Some("1e3"),
            Some(&*"9".repeat(33)),
        ] {
            let transport = ScriptedTransport::responses(vec![
                busy(429, header),
                busy(429, header),
                busy(429, header),
                busy(429, header),
                ok(),
            ]);
            let client = client(&transport, ResendPolicy::default());
            let (result, holdoffs) = drive(&client, &request(), &ManualClock::new(Instant::now()));
            assert!(result.is_ok(), "{header:?}: {result:?}");
            assert_eq!(holdoffs.len(), 4);
            for (holdoff, base) in holdoffs.iter().zip([1.0, 2.0, 4.0, 8.0]) {
                assert_between(*holdoff, base, base + 0.25);
            }
        }
        // Doubling continues to the 30 s cap.
        let transport = ScriptedTransport::responses((0..7).map(|_| busy(529, None)).collect());
        let client = client(&transport, policy(7));
        let (_, holdoffs) = drive(&client, &request(), &ManualClock::new(Instant::now()));
        let whole: Vec<u64> = holdoffs.iter().map(Duration::as_secs).collect();
        assert_eq!(whole, [1, 2, 4, 8, 16, 30]);
        assert_between(holdoffs[5], 30.0, 30.25);
    }

    // A5-04: the 90 s start budget refuses the next resend.
    #[test]
    fn budget_stops_next_start() {
        let transport =
            ScriptedTransport::responses((0..10).map(|_| busy(429, Some("30"))).collect());
        let client = client(&transport, policy(10));
        let clock = ManualClock::new(Instant::now());
        let start = clock.now();
        let (result, _) = drive(&client, &request(), &clock);
        match result {
            Err(HttpError::ProviderBusy {
                reason: BusyReason::Budget,
                attempts: 3,
                retry_after_seconds: Some(seconds),
                ..
            }) => assert_eq!(seconds, 30.0),
            other => panic!("expected provider_busy (budget, 3): {other:?}"),
        }
        assert_eq!(transport.sends(), 3);
        // The third holdoff was cut to the budget left: the resend was asked for
        // at exactly 90 s after the first send.
        assert_eq!(clock.now() - start, secs(90.0));
    }

    // A5-04: may_start after a final reply refuses with a reason and no HttpError.
    #[test]
    fn may_start_after_done_refused() {
        let request = request();
        // Sends: max_sends 1 used by the first send.
        let transport = ScriptedTransport::responses(vec![ok(), ok()]);
        let client = client(&transport, policy(1));
        let clock = ManualClock::new(Instant::now());
        let (result, _) = drive(&client, &request, &clock);
        let (_, attempts) = result.expect("200");
        assert_eq!(attempts.sends(), 1);
        assert!(matches!(
            client.may_start(&request, attempts, clock.now()),
            Err(BusyReason::Sends)
        ));
        // Budget: 90 s after the first send.
        let client = self::client(&transport, ResendPolicy::default());
        let (result, _) = drive(&client, &request, &clock);
        let (_, attempts) = result.expect("200");
        assert!(matches!(
            client.may_start(&request, attempts.clone(), clock.now() + secs(89.9)),
            Ok(Start::Go(_))
        ));
        assert!(matches!(
            client.may_start(&request, attempts, clock.now() + secs(90.0)),
            Err(BusyReason::Budget)
        ));
        assert_eq!(transport.sends(), 2);
    }

    // A5-04 / REQ-SAG-0007
    #[test]
    fn retry_after_unrounded() {
        let transport =
            ScriptedTransport::responses(vec![busy(429, Some("1.5")), busy(429, Some("2.25"))]);
        let client = client(&transport, policy(2));
        let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
        match result {
            Err(HttpError::ProviderBusy {
                retry_after_seconds,
                last,
                ..
            }) => {
                assert_eq!(retry_after_seconds, Some(2.25));
                assert_eq!(last.retry_after_seconds, Some(2.25));
            }
            other => panic!("{other:?}"),
        }
    }

    // A5-04: a busy request is resent unchanged.
    #[test]
    fn resend_same_bytes() {
        let transport = ScriptedTransport::new(vec![
            Ok(busy(429, Some("1"))),
            Err(timeout()),
            Ok(busy(529, None)),
            Ok(ok()),
        ]);
        let client = client(&transport, ResendPolicy::default());
        let request = request();
        let (result, _) = drive(&client, &request, &ManualClock::new(Instant::now()));
        assert!(result.is_ok(), "{result:?}");
        let sent = transport.requests();
        assert_eq!(sent.len(), 4);
        for each in &sent {
            assert_eq!(each.body(), request.body());
            assert_eq!(each.headers(), request.headers());
            assert_eq!(each.endpoint(), request.endpoint());
            assert_eq!(each.method(), request.method());
            assert_eq!(
                each.secret().map(SecretHeader::redacted),
                request.secret().map(SecretHeader::redacted)
            );
            assert_eq!(each.deadline(), request.deadline());
        }
    }

    // A5-05: the holdoff comes back as a not-before instant; send_once never waits.
    #[test]
    fn holdoff_is_resend_at() {
        let transport = ScriptedTransport::responses(vec![busy(429, Some("30"))]);
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let before = clock.now();
        let request = request();
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), clock.now()) else {
            panic!("fresh");
        };
        let wall = Instant::now();
        let next = client.send_once(&request, permit, LogContext::NONE, &|| clock.now());
        // The call returned well inside the 30 s holdoff it reports.
        assert!(wall.elapsed() < secs(30.0));
        assert_eq!(clock.now(), before);
        match next {
            Ok(Next::ResendAt { at, pending }) => {
                assert!(at >= before + secs(30.0));
                assert_eq!(pending.attempts().sends(), 1);
            }
            other => panic!("{other:?}"),
        }
    }

    // A5-06: a timeout is resent unchanged and counts toward max_sends.
    #[test]
    fn timeout_resent_under_max_sends() {
        let transport = ScriptedTransport::new((0..8).map(|_| Err(timeout())).collect());
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let (result, holdoffs) = drive(&client, &request(), &clock);
        match result {
            Err(HttpError::Network {
                kind: NetworkKind::Timeout,
                attempts: 5,
                ..
            }) => {}
            other => panic!("expected timeout after 5 sends: {other:?}"),
        }
        assert_eq!(transport.sends(), 5);
        // Resent after only 0-250 ms of jitter (RSH-003), and the host is not paused.
        assert_eq!(holdoffs.len(), 4);
        assert!(
            holdoffs.iter().all(|holdoff| *holdoff <= secs(0.25)),
            "{holdoffs:?}"
        );
        assert_eq!(client.pause().not_before("api.example.test"), None);
    }

    // A5-06: a 120 s timeout uses the budget; the request ends with the timeout
    // error, never provider_busy.
    #[test]
    fn timeout_then_budget_ends_with_timeout() {
        let clock = ManualClock::new(Instant::now());
        let transport = ScriptedTransport::new(vec![Err(timeout()), Ok(ok())])
            .with_send_duration(clock.clone(), Duration::from_secs(120));
        let client = client(&transport, ResendPolicy::default());
        let (result, _) = drive(&client, &request(), &clock);
        match result {
            Err(
                error @ HttpError::Network {
                    kind: NetworkKind::Timeout,
                    attempts: 1,
                    ..
                },
            ) => assert_eq!(sc_agent_types::ErrorId::id(&error), "http.network"),
            other => panic!("expected the timeout error: {other:?}"),
        }
        assert_eq!(transport.sends(), 1);
    }

    // A5-06 (RSH-001): real default Timeouts values.
    #[test]
    fn default_timeouts_which_resend() {
        let defaults = sc_agent_http::Timeouts::default();
        for (per_send, sends) in [(defaults.connect, 5), (defaults.request, 1)] {
            let clock = ManualClock::new(Instant::now());
            let transport = ScriptedTransport::new((0..8).map(|_| Err(timeout())).collect())
                .with_send_duration(clock.clone(), per_send);
            let client = client(&transport, ResendPolicy::default());
            let (result, _) = drive(&client, &request(), &clock);
            assert!(
                matches!(result, Err(HttpError::Network { kind: NetworkKind::Timeout, attempts, .. }) if attempts == sends),
                "{per_send:?}: {result:?}"
            );
            assert_eq!(transport.sends(), sends as usize, "{per_send:?}");
        }
    }

    // A5-06: connect, TLS and other transport failures are returned after one send.
    #[test]
    fn connect_not_resent() {
        for kind in [NetworkKind::Connect, NetworkKind::Tls, NetworkKind::Other] {
            let error = HttpError::network(kind, 1, "ureq.HostNotFound: host not found".to_owned());
            let transport = ScriptedTransport::new(vec![Err(error), Ok(ok())]);
            let client = client(&transport, ResendPolicy::default());
            let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
            assert!(
                matches!(result, Err(HttpError::Network { kind: got, attempts: 1, .. }) if got == kind),
                "{kind:?}: {result:?}"
            );
            assert_eq!(transport.sends(), 1);
        }
    }

    const ANTHROPIC_SPEND_CAP: &str = r#"{"type":"error","error":{"type":"rate_limit_error","message":"You have reached your API usage limits","details":{"error_code":"enforced_spend_limit_reached"}},"request_id":"req_cap"}"#;
    const OPENAI_QUOTA: &str = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","param":null,"code":"insufficient_quota"}}"#;
    const ANTHROPIC_USAGE_LIMIT_400: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"You have reached your specified API usage limits. You will regain access on 2026-12-01."},"request_id":"req_400"}"#;
    const ANTHROPIC_BILLING_402: &str = r#"{"type":"error","error":{"type":"billing_error","message":"Your credit balance is too low"},"request_id":"req_402"}"#;

    fn reply(status: u16, body: &str) -> HttpResponse {
        HttpResponse::new(
            status,
            vec![("request-id".to_owned(), "hdr".to_owned())],
            body.as_bytes().to_vec(),
        )
    }

    // A5-07: 402 and Anthropic's usage-limit 400 are final after one send.
    #[test]
    fn final_402_and_usage_400() {
        for response in [
            reply(402, ANTHROPIC_BILLING_402),
            reply(400, ANTHROPIC_USAGE_LIMIT_400),
            reply(402, OPENAI_QUOTA),
        ] {
            let transport = ScriptedTransport::responses(vec![response.clone(), ok()]);
            let client = client(&transport, ResendPolicy::default());
            let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
            let (got, attempts) = result.expect("final reply");
            assert_eq!(
                got, response,
                "the remote id and message come back unchanged"
            );
            assert_eq!(attempts.sends(), 1);
            assert_eq!(transport.sends(), 1);
        }
    }

    // A5-08: Anthropic's documented quota-exhausted 429.
    #[test]
    fn quota_429_is_final() {
        let transport = ScriptedTransport::responses(vec![reply(429, ANTHROPIC_SPEND_CAP), ok()]);
        let client = client(&transport, ResendPolicy::default());
        let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
        match result {
            Err(HttpError::QuotaExhausted { attempts: 1, reply }) => {
                assert_eq!(reply.status, 429);
                assert_eq!(reply.provider_type.as_deref(), Some("rate_limit_error"));
                assert_eq!(reply.request_id.as_deref(), Some("req_cap"));
                assert_eq!(reply.message, "You have reached your API usage limits");
            }
            other => panic!("expected quota_exhausted: {other:?}"),
        }
        assert_eq!(transport.sends(), 1);
    }

    // A5-08: OpenAI's insufficient_quota 429.
    #[test]
    fn openai_quota_429_is_final() {
        let transport = ScriptedTransport::responses(vec![reply(429, OPENAI_QUOTA), ok()]);
        let client = client(&transport, ResendPolicy::default());
        let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
        match result {
            Err(HttpError::QuotaExhausted { attempts: 1, reply }) => {
                assert_eq!(reply.provider_code.as_deref(), Some("insufficient_quota"));
                assert_eq!(reply.message, "You exceeded your current quota");
            }
            other => panic!("expected quota_exhausted: {other:?}"),
        }
        assert_eq!(transport.sends(), 1);
    }

    // A5-08: a 429 with an unrecognized body is busy.
    #[test]
    fn rate_limit_429_resends() {
        for body in [
            r#"{"error":{"message":"Rate limit reached","type":"requests","code":"rate_limit_exceeded"}}"#,
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#,
            "not json",
            "",
        ] {
            assert!(first_holdoff(reply(429, body)) >= secs(1.0), "{body}");
        }
    }

    // A5-09
    #[test]
    fn final_replies_unchanged() {
        for status in [200, 201, 400, 401, 402, 404, 413, 422, 500, 503, 504] {
            let response = reply(status, r#"{"error":{"message":"m","type":"t"}}"#);
            let transport = ScriptedTransport::responses(vec![response.clone()]);
            let client = client(&transport, ResendPolicy::default());
            let (result, _) = drive(&client, &request(), &ManualClock::new(Instant::now()));
            assert_eq!(result.map(|(got, _)| got).ok(), Some(response), "{status}");
            assert_eq!(transport.sends(), 1);
        }
    }

    // A5-21: a busy reply pauses the host for every request of the client.
    #[test]
    fn busy_pauses_host() {
        let transport = ScriptedTransport::responses(vec![busy(429, Some("5"))]);
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let first = request();
        let Ok(Start::Go(permit)) = client.may_start(&first, Attempts::new(), clock.now()) else {
            panic!("fresh");
        };
        let Ok(Next::ResendAt { pending, .. }) =
            client.send_once(&first, permit, LogContext::NONE, &|| clock.now())
        else {
            panic!("busy");
        };
        let paused_until = client
            .pause()
            .not_before("api.example.test")
            .expect("host paused");
        assert!(paused_until >= clock.now() + secs(5.0));
        // Another request to the same host waits until the pause ends.
        let other = request_to("https://api.example.test/v1/other");
        match client.may_start(&other, Attempts::new(), clock.now()) {
            Ok(Start::Wait { at, state }) => {
                assert_eq!(at, paused_until);
                assert_eq!(state.sends(), 0);
            }
            other => panic!("expected Wait: {other:?}"),
        }
        // A held resend asked early also waits; after the pause it may go.
        let pending = match client.may_resend(&first, pending, clock.now()) {
            Ok(Start::Wait { at, state }) => {
                assert_eq!(at, paused_until);
                state
            }
            other => panic!("expected Wait: {other:?}"),
        };
        assert!(matches!(
            client.may_resend(&first, pending, paused_until),
            Ok(Start::Go(_))
        ));
        assert!(matches!(
            client.may_start(&other, Attempts::new(), paused_until),
            Ok(Start::Go(_))
        ));
    }

    // A5-21
    #[test]
    fn pause_is_per_host() {
        let transport = ScriptedTransport::responses(vec![busy(529, Some("10"))]);
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let request = request();
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), clock.now()) else {
            panic!("fresh");
        };
        let _held = client.send_once(&request, permit, LogContext::NONE, &|| clock.now());
        let elsewhere = request_to("https://api.other.test/v1/x");
        assert!(matches!(
            client.may_start(&elsewhere, Attempts::new(), clock.now()),
            Ok(Start::Go(_))
        ));
        assert_eq!(client.pause().not_before("api.other.test"), None);
    }

    // A5-21
    #[test]
    fn pause_shared_by_clones() {
        let transport = ScriptedTransport::responses(vec![busy(502, None)]);
        let client = client(&transport, ResendPolicy::default());
        let twin = client.clone();
        let clock = ManualClock::new(Instant::now());
        let request = request();
        let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), clock.now()) else {
            panic!("fresh");
        };
        let _held = client.send_once(&request, permit, LogContext::NONE, &|| clock.now());
        assert!(matches!(
            twin.may_start(&request, Attempts::new(), clock.now()),
            Ok(Start::Wait { .. })
        ));
        assert_eq!(
            twin.pause().not_before("api.example.test"),
            client.pause().not_before("api.example.test")
        );
    }

    // A5-26: a load wait before the first send never counts toward the budget.
    #[test]
    fn budget_starts_at_first_send() {
        let clock = ManualClock::new(Instant::now());
        clock.advance(Duration::from_secs(600));
        let transport = ScriptedTransport::responses(vec![busy(429, Some("1")), ok()]);
        let client = client(&transport, ResendPolicy::default());
        let request = request().with_deadline(Duration::from_secs(900));
        let (result, holdoffs) = drive(&client, &request, &clock);
        let (_, attempts) = result.expect("resent after the busy reply");
        assert_eq!(attempts.sends(), 2);
        assert_between(holdoffs[0], 1.0, 1.25);
        assert!(
            transport
                .requests()
                .iter()
                .all(|sent| sent.deadline() == Some(Duration::from_secs(900)))
        );
    }

    // QA-001 (REQ-SAG-0012): the busy reply on the last allowed send pauses
    // the host too.
    #[test]
    fn last_send_busy_pauses_host() {
        let transport = ScriptedTransport::responses(vec![busy(429, Some("5"))]);
        let client = client(&transport, policy(1));
        let clock = ManualClock::new(Instant::now());
        let (result, _) = drive(&client, &request(), &clock);
        assert!(
            matches!(
                result,
                Err(HttpError::ProviderBusy {
                    reason: BusyReason::Sends,
                    attempts: 1,
                    ..
                })
            ),
            "{result:?}"
        );
        let other = request_to("https://api.example.test/v1/other");
        match client.may_start(&other, Attempts::new(), clock.now()) {
            Ok(Start::Wait { at, .. }) => assert_between(at - clock.now(), 5.0, 5.25),
            other => panic!("expected Wait: {other:?}"),
        }
    }

    // QA-001: the fifth busy reply under the default policy pauses the host.
    #[test]
    fn fifth_busy_pauses_host() {
        let transport =
            ScriptedTransport::responses((0..5).map(|_| busy(529, Some("7"))).collect());
        let client = client(&transport, ResendPolicy::default());
        let clock = ManualClock::new(Instant::now());
        let (result, _) = drive(&client, &request(), &clock);
        assert!(
            matches!(
                result,
                Err(HttpError::ProviderBusy {
                    reason: BusyReason::Sends,
                    attempts: 5,
                    ..
                })
            ),
            "{result:?}"
        );
        assert_eq!(transport.sends(), 5);
        let paused = client
            .pause()
            .not_before("api.example.test")
            .expect("paused after the fifth reply");
        assert_between(paused - clock.now(), 7.0, 7.25);
    }

    // RSH-004: a zero or over-long deadline is refused before any send.
    #[test]
    fn invalid_deadline_refused_before_send() {
        for deadline in [Duration::ZERO, Duration::from_secs(3601)] {
            let transport = ScriptedTransport::responses(vec![ok()]);
            let client = client(&transport, ResendPolicy::default());
            let request = request().with_deadline(deadline);
            let Ok(Start::Go(permit)) = client.may_start(&request, Attempts::new(), Instant::now())
            else {
                panic!("fresh");
            };
            let result = client.send_once(&request, permit, LogContext::NONE, &Instant::now);
            assert!(
                matches!(result, Err(HttpError::InvalidRequest { .. })),
                "{deadline:?}: {result:?}"
            );
            assert_eq!(transport.sends(), 0);
        }
    }
}
