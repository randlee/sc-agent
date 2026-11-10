//! Provider choice, catalog, busy/quota/pause, config bounds, limits and local runners,
//! end to end through `decide::send` / `send_batch` and the real runner.

mod common;

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::time::Duration;

    use decide::input::{
        BatchInput, MAX_IMAGE_BYTES, MAX_REQUEST_BYTES, build_batch, build_request, read_batch,
        request_bytes,
    };
    use decide::{DecideError, load_config, send, send_batch};
    use sc_agent_http::HttpResponse;
    use sc_agent_http::testing::{BlockingTransport, ScriptedTransport};
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_types::{CancelToken, ErrorId, Provider};
    use sc_decision_schema::{DecisionError, DecisionRequest};

    use crate::common::{
        ANTHROPIC_KEY, NOUL, OPENROUTER_KEY, TYPESAFE_KEY, TimedTransport, anthropic_noul,
        body_json, config_dir, context, empty_config_dir, json_response, ok_noul, options, request,
    };

    fn haiku_send(
        vars: &[(&str, &str)],
        provider: Option<Provider>,
        config: Option<&str>,
    ) -> (
        Result<sc_decision_schema::DecisionResult, DecideError>,
        ScriptedTransport,
    ) {
        let transport = ScriptedTransport::responses(vec![anthropic_noul()]);
        let log = MemoryLog::new().expect("log");
        let dir = config.map_or_else(empty_config_dir, config_dir);
        let ctx = context(dir.path(), vars, Arc::new(transport.clone()), &log);
        let options = options("haiku", provider, ctx, &log);
        (
            send(&options, request(NOUL), &CancelToken::new()),
            transport,
        )
    }

    /// AC: REQ-SAG-0103. haiku with Anthropic and OpenRouter keys goes to Anthropic; with only
    /// OpenRouter to OpenRouter as `anthropic/claude-haiku-5.5`; `--provider openrouter`
    /// overrides; jev with no TypeSafe key is `decide.no_provider_key` with a recovery.
    #[test]
    fn the_provider_is_chosen_by_ranking_among_detected_keys() {
        let both = [
            (sc_agent_secrets::ANTHROPIC_API_KEY, ANTHROPIC_KEY),
            (sc_agent_secrets::OPENROUTER_API_KEY, OPENROUTER_KEY),
        ];
        let (result, transport) = haiku_send(&both, None, None);
        assert!(result.expect("send").outcome.is_ok());
        let sent = transport.requests();
        assert_eq!(sent[0].endpoint().host(), "api.anthropic.com");
        assert_eq!(body_json(&sent[0])["model"], "claude-haiku-5-5");

        let (_, transport) = haiku_send(
            &[(sc_agent_secrets::OPENROUTER_API_KEY, OPENROUTER_KEY)],
            None,
            None,
        );
        let sent = transport.requests();
        assert_eq!(sent[0].endpoint().host(), "openrouter.ai");
        assert_eq!(body_json(&sent[0])["model"], "anthropic/claude-haiku-5.5");

        let (_, transport) = haiku_send(&both, Some(Provider::OpenRouter), None);
        assert_eq!(transport.requests()[0].endpoint().host(), "openrouter.ai");

        let transport = ScriptedTransport::responses(vec![ok_noul()]);
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(dir.path(), &both, Arc::new(transport.clone()), &log);
        let error = send(
            &options("jev", None, ctx, &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect_err("no key");
        assert_eq!(error.id(), "decide.no_provider_key");
        assert!(error.recovery().is_some());
        assert_eq!(transport.sends(), 0);
    }

    /// AC: REQ-SAG-0101 / REQ-SAG-0102. `--model haiku` resolves through the config-built
    /// catalog: a later-dated config haiku model is sent; without it, the built-in.
    #[test]
    fn haiku_resolves_through_the_config_catalog() {
        let keys = [(sc_agent_secrets::ANTHROPIC_API_KEY, ANTHROPIC_KEY)];
        let config = r#"{"models":{"claude-haiku-6-0":{"line":"haiku","released":"2026-12-01"}}}"#;
        let (_, transport) = haiku_send(&keys, None, Some(config));
        assert_eq!(
            body_json(&transport.requests()[0])["model"],
            "claude-haiku-6-0"
        );
        let (_, transport) = haiku_send(&keys, None, None);
        assert_eq!(
            body_json(&transport.requests()[0])["model"],
            "claude-haiku-5-5"
        );
    }

    /// AC: REQ-SAG-0602. An Anthropic 429 with the quota body ends with
    /// `http.quota_exhausted` after one send, with the remote message and its recovery.
    #[test]
    fn an_anthropic_quota_reply_is_passed_through_after_one_send() {
        let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"You have reached your spend limit","details":{"error_code":"enforced_spend_limit_reached"}},"request_id":"req_quota_1"}"#;
        let transport =
            ScriptedTransport::responses(vec![json_response(429, body), anthropic_noul()]);
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::ANTHROPIC_API_KEY, ANTHROPIC_KEY)],
            Arc::new(transport.clone()),
            &log,
        );
        let result = send(
            &options("haiku", None, ctx, &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect("send");
        assert_eq!(transport.sends(), 1);
        let error = result.outcome.expect_err("quota");
        let detail = error.detail();
        assert_eq!(detail.id, "decision.http");
        let cause = detail.cause.expect("cause");
        assert_eq!(cause.id, "http.quota_exhausted");
        assert!(
            cause.message.contains("You have reached your spend limit"),
            "{}",
            cause.message
        );
        assert!(
            cause.recovery.is_some(),
            "the quota error carries its recovery"
        );
        let line = String::from_utf8(decide::output::result_line(&result_of(error)).expect("line"))
            .expect("utf-8");
        assert!(line.contains("http.quota_exhausted"));
    }

    fn result_of(error: DecisionError) -> sc_decision_schema::DecisionResult {
        let request = request(NOUL);
        sc_decision_schema::DecisionResult::new(
            sc_agent_types::SessionId::new(),
            0,
            request.digests().clone(),
            Err(error),
        )
    }

    fn busy() -> HttpResponse {
        json_response(
            529,
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        )
    }

    /// AC: REQ-SAG-0601. A request answered 529 (no Retry-After) every time is sent 5 times
    /// and ends `http.provider_busy` (reason `sends`, 5 attempts); `SC_AGENT__MAX_SENDS=2` in
    /// the context environment sends it twice. Real clock: the backoff is 1+2+4+8 s.
    #[test]
    fn a_busy_provider_is_resent_up_to_max_sends() {
        for (vars, sends) in [
            (
                vec![(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                5_usize,
            ),
            (
                vec![
                    (sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY),
                    ("SC_AGENT__MAX_SENDS", "2"),
                ],
                2,
            ),
        ] {
            let transport = ScriptedTransport::responses((0..10).map(|_| busy()).collect());
            let log = MemoryLog::new().expect("log");
            // An override needs a default.json (A-4: an override is never silently ignored).
            let dir = config_dir("{}");
            let ctx = context(dir.path(), &vars, Arc::new(transport.clone()), &log);
            let result = send(
                &options("jev", None, ctx, &log),
                request(NOUL),
                &CancelToken::new(),
            )
            .expect("send");
            assert_eq!(transport.sends(), sends);
            let detail = result.outcome.expect_err("busy").detail();
            let cause = detail.cause.expect("cause");
            assert_eq!(cause.id, "http.provider_busy");
            assert!(cause.message.contains("sends"), "{}", cause.message);
            assert!(
                cause.message.contains(&sends.to_string()),
                "{}",
                cause.message
            );
        }
    }

    /// Runs two requests as one batch over `transport`, with `config` as default.json.
    fn paused_batch(transport: &TimedTransport, config: &str) {
        let log = MemoryLog::new().expect("log");
        let dir = config_dir(config);
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport.clone()),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        let requests: Vec<DecisionRequest> = (0..2).map(|_| request(NOUL)).collect();
        let mut ok = 0;
        send_batch(&options, requests, &CancelToken::new(), &mut |result| {
            ok += usize::from(result.outcome.is_ok());
            Ok(())
        })
        .expect("batch");
        assert_eq!(ok, 2);
    }

    fn retry_after(seconds: &str) -> HttpResponse {
        HttpResponse::new(
            429,
            vec![("retry-after".to_owned(), seconds.to_owned())],
            br#"{"detail":"slow down"}"#.to_vec(),
        )
    }

    /// AC: REQ-SAG-0012. A 429 with `Retry-After: 1` on the first send pauses every later send
    /// to that host until the not-before time, the other request's included, because the
    /// pause lives in the one `HttpClient` the runner shares; `Retry-After: 0` still waits the
    /// 250 ms floor. Real clock: the runner's clock seam is crate-private to
    /// sc-decision-runner, so the waits are real (about 1.25 s each).
    ///
    /// With the default 8 workers both requests may be sent before the 429 returns, so that
    /// run checks every send that starts after the 429. With `workers: 1` the other request
    /// can only start after the 429: three sends are made (the 429, the other request's first
    /// send, the resend), and both later ones wait out the pause, which proves that the pause
    /// holds the other request too.
    #[test]
    fn a_retry_after_pauses_every_send_to_the_host() {
        for (seconds, floor) in [
            ("1", Duration::from_secs(1)),
            ("0", Duration::from_millis(250)),
        ] {
            // Default workers: two parallel requests.
            let transport = TimedTransport::new(vec![Ok(retry_after(seconds)), Ok(ok_noul())]);
            paused_batch(&transport, "{}");
            let sends = transport.sends();
            let busy_at = sends[0].0;
            let after: Vec<Duration> = sends
                .iter()
                .skip(1)
                .map(|(at, _)| at.duration_since(busy_at))
                .filter(|since| *since > Duration::from_millis(50))
                .collect();
            assert!(!after.is_empty(), "the resend comes after the 429");
            for since in after {
                assert!(
                    since >= floor,
                    "a send {since:?} after the 429 ignored the {floor:?} pause"
                );
            }

            // One worker: the other request starts after the 429 and is paused too.
            let transport = TimedTransport::new(vec![Ok(retry_after(seconds)), Ok(ok_noul())]);
            paused_batch(&transport, r#"{"workers":1}"#);
            let sends = transport.sends();
            assert_eq!(
                sends.len(),
                3,
                "the 429, the other request's send, the resend"
            );
            for (at, _) in &sends[1..] {
                let since = at.duration_since(sends[0].0);
                assert!(
                    since >= floor,
                    "a send {since:?} after the 429 ignored the {floor:?} pause"
                );
            }
        }
    }

    /// Send 0 answers the 429 and marks when it returned; send 1 is held until then (so its
    /// worker stays busy past the 429); every later send answers 200. Send start times are
    /// recorded.
    #[derive(Clone)]
    struct Gated {
        retry_after: &'static str,
        state: Arc<(std::sync::Mutex<GatedState>, std::sync::Condvar)>,
    }

    #[derive(Default)]
    struct GatedState {
        starts: Vec<std::time::Instant>,
        busy_returned: Option<std::time::Instant>,
    }

    impl sc_agent_http::Transport for Gated {
        fn send(
            &self,
            _request: &sc_agent_http::HttpRequest,
        ) -> Result<HttpResponse, sc_agent_http::HttpError> {
            let (lock, condvar) = &*self.state;
            let mut state = lock.lock().expect("lock");
            let index = state.starts.len();
            state.starts.push(std::time::Instant::now());
            match index {
                0 => {
                    state.busy_returned = Some(std::time::Instant::now());
                    condvar.notify_all();
                    Ok(retry_after(self.retry_after))
                }
                1 => {
                    while state.busy_returned.is_none() {
                        state = condvar.wait(state).expect("wait");
                    }
                    Ok(ok_noul())
                }
                _ => Ok(ok_noul()),
            }
        }
    }

    /// AC: REQ-SAG-0012 with `workers: 2` (REQ-QA-004). Two workers take the first two
    /// requests; the second one's send is held until the 429 has returned, so the third
    /// request can only be admitted after the 429. Its first send, and the resend, both wait
    /// out the pause the 429 set on the shared client.
    #[test]
    fn with_two_workers_a_request_admitted_after_the_429_waits_the_pause() {
        for (seconds, floor) in [
            ("1", Duration::from_secs(1)),
            ("0", Duration::from_millis(250)),
        ] {
            let transport = Gated {
                retry_after: seconds,
                state: Arc::default(),
            };
            let log = MemoryLog::new().expect("log");
            let dir = config_dir(r#"{"workers":2}"#);
            let ctx = context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            );
            let options = options("jev", None, ctx, &log);
            let requests: Vec<DecisionRequest> = (0..3).map(|_| request(NOUL)).collect();
            let mut ok = 0;
            send_batch(&options, requests, &CancelToken::new(), &mut |result| {
                ok += usize::from(result.outcome.is_ok());
                Ok(())
            })
            .expect("batch");
            assert_eq!(ok, 3);
            let state = transport.state.0.lock().expect("lock");
            let busy = state.busy_returned.expect("the 429 was sent");
            assert_eq!(
                state.starts.len(),
                4,
                "the 429, two first sends, the resend"
            );
            for start in &state.starts[2..] {
                let since = start.duration_since(busy);
                assert!(
                    since >= floor,
                    "a send {since:?} after the 429 ignored the {floor:?} pause"
                );
            }
        }
    }

    /// AC: REQ-SAG-0013. Bound violations fail before any send with `decide.config`.
    #[test]
    fn config_bounds_fail_before_any_send() {
        let key_on_plain_http = format!(
            r#"{{"local_runners":{{"laya":{{"kind":"laya","host":"10.0.0.5","port":9000,"model":"m","key_env":"{}"}}}}}}"#,
            sc_agent_secrets::LAYA_API_KEY
        );
        let cases: [(&str, &str, &str); 4] = [
            (r#"{"workers":0}"#, "config.invalid", "workers"),
            (r#"{"max_sends":11}"#, "config.invalid", "max_sends"),
            (
                r#"{"local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":0,"model":"m"}}}"#,
                "config.invalid",
                "local_runners.laya.port",
            ),
            (
                &key_on_plain_http,
                "config.key_on_plain_http",
                "local_runners.laya.key_env",
            ),
        ];
        for (json, cause, path) in cases {
            let transport = ScriptedTransport::responses(vec![ok_noul()]);
            let log = MemoryLog::new().expect("log");
            let dir = config_dir(json);
            let ctx = context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            );
            let error = load_config(&ctx).expect_err(json);
            assert_eq!(error.id(), "decide.config");
            let detail = error.cause_detail().expect("cause");
            assert_eq!(detail.id, cause, "{json}");
            assert!(detail.message.contains(path), "{json}: {}", detail.message);
            // send loads the same config first: nothing is sent.
            let options = decide::SendOptions {
                model: decide::ModelSelector::Short(sc_agent_types::ShortName::Jev),
                provider: None,
                project: sc_agent_log::Slug::new("project", "proj").expect("slug"),
                agent_role: sc_agent_log::Slug::new("agent_role", "role").expect("slug"),
                context: ctx,
                session: Some(log.session(crate::common::identity())),
            };
            let error = send(&options, request(NOUL), &CancelToken::new()).expect_err(json);
            assert_eq!(error.id(), "decide.config");
            assert_eq!(transport.sends(), 0);
        }
    }

    /// AC: REQ-SAG-0014. Each limit ends with a typed error naming it; nothing is sent.
    #[test]
    fn input_limits_are_typed_errors() {
        let limit = |error: DecideError| match error {
            DecideError::LimitExceeded { what, .. } => what.as_str().to_owned(),
            DecideError::RequestInvalid { source, .. } => source.id().to_owned(),
            other => panic!("unexpected {other:?}"),
        };
        // A 9 MiB single request.
        let big = vec![b' '; 9 * 1024 * 1024];
        assert_eq!(
            limit(request_bytes("-", &mut Cursor::new(big.clone())).expect_err("9 MiB")),
            "request_bytes"
        );
        // A 10,001-line batch.
        let lines: String = (0..10_001).map(|_| format!("{NOUL}\n")).collect();
        assert_eq!(
            limit(read_batch(&mut Cursor::new(lines)).expect_err("10,001 lines")),
            "batch_lines"
        );
        // A line past 8 MiB.
        let mut long = vec![b' '; MAX_REQUEST_BYTES + 1];
        long.push(b'\n');
        let error = read_batch(&mut Cursor::new(long)).expect_err("long line");
        assert!(matches!(
            error,
            DecideError::LimitExceeded {
                what: decide::input::LimitKind::RequestBytes,
                line: Some(1),
                ..
            }
        ));
        // A 21 MiB image (sparse file: the size is checked from metadata before reading).
        let dir = tempfile::tempdir().expect("dir");
        let image = dir.path().join("big.png");
        std::fs::File::create(&image)
            .expect("create")
            .set_len(MAX_IMAGE_BYTES + 1024 * 1024)
            .expect("len");
        let with_part = r#"{"model":"jev-latest","state":{"d":"a"},"assets":{"a":{"type":"image","part":"p"}},"questions":{"q":{"type":"noul"}}}"#;
        let error = build_request(with_part.as_bytes(), &[("p".into(), image)], false, None)
            .expect_err("21 MiB");
        assert_eq!(limit(error), "image_bytes");
        // 21 images.
        let images: Vec<(String, std::path::PathBuf)> = (0..21)
            .map(|index| (format!("p{index}"), dir.path().join(format!("{index}.png"))))
            .collect();
        let error =
            build_request(with_part.as_bytes(), &images, false, None).expect_err("21 images");
        assert_eq!(error.id(), "decide.request_invalid");
        assert_eq!(limit(error), "decision.limits");
        // In memory (sc-agent-py): 10,001 items and a request over 8 MiB.
        let items: Vec<BatchInput> = (0..10_001)
            .map(|_| BatchInput {
                json: NOUL.to_owned(),
                ..BatchInput::default()
            })
            .collect();
        assert_eq!(
            limit(build_batch(items).expect_err("10,001 items")),
            "batch_lines"
        );
        let error =
            decide::input::build_request_bytes(&big, Vec::new(), false, None).expect_err("9 MiB");
        assert_eq!(limit(error), "request_bytes");
        let png = [
            &[0x89_u8, b'P', b'N', b'G'][..],
            &vec![0_u8; 21 * 1024 * 1024],
        ]
        .concat();
        let error = decide::input::build_request_bytes(
            with_part.as_bytes(),
            vec![("p".into(), png)],
            false,
            None,
        )
        .expect_err("21 MiB");
        assert_eq!(limit(error), "image_bytes");
    }

    /// AC: REQ-SAG-0014 (REQ-QA-002). Past 256 MiB a batch is `batch_bytes` and nothing is
    /// sent: from a file (generated, never held) through `read_batch`, and in memory through
    /// `build_batch` (10,001 items, the line cap; the byte cap over item sizes is unit-tested
    /// in `input.rs` without allocating 256 MiB).
    #[test]
    fn an_oversized_batch_sends_nothing() {
        let transport = ScriptedTransport::responses(vec![ok_noul()]);
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport.clone()),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        let outcome = read_batch(&mut crate::common::BlankLines::new(1024 * 1024, 300)).and_then(
            |requests| send_batch(&options, requests, &CancelToken::new(), &mut |_| Ok(())),
        );
        assert!(matches!(
            outcome,
            Err(DecideError::LimitExceeded {
                what: decide::input::LimitKind::BatchBytes,
                ..
            })
        ));
        let items: Vec<BatchInput> = (0..10_001)
            .map(|_| BatchInput {
                json: NOUL.to_owned(),
                ..BatchInput::default()
            })
            .collect();
        let outcome = build_batch(items).and_then(|requests| {
            send_batch(&options, requests, &CancelToken::new(), &mut |_| Ok(()))
        });
        assert!(matches!(
            outcome,
            Err(DecideError::LimitExceeded {
                what: decide::input::LimitKind::BatchLines,
                ..
            })
        ));
        assert_eq!(transport.sends(), 0);
    }

    /// AC: REQ-SAG-0801. `local_runners.laya` (127.0.0.1:9000) makes `--model laya` build a
    /// LocalBackend that sends to the configured host and port.
    #[test]
    fn a_local_runner_is_selected_by_its_configured_name() {
        let dir = config_dir(
            r#"{"local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"jev-latest"}}}"#,
        );
        let transport = ScriptedTransport::responses(vec![
            json_response(200, r#"{"status":"ok","loaded":["jev-latest"]}"#),
            ok_noul(),
        ]);
        let log = MemoryLog::new().expect("log");
        let ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
        let result = send(
            &options("laya", None, ctx, &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect("send");
        assert!(result.outcome.is_ok(), "{:?}", result.outcome);
        let sent = transport.requests();
        assert_eq!(sent[0].endpoint().as_str(), "http://127.0.0.1:9000/health");
        assert_eq!(
            sent[1].endpoint().as_str(),
            "http://127.0.0.1:9000/v1/systemone"
        );
    }

    /// AC: REQ-SAG-0015. A scripted daemon reports not-loaded twice, then loaded: decide
    /// probes until loaded, then makes one send; the send carries no load deadline (its
    /// timeout is the runner transport's own, counted from the send, after the load). A
    /// cancel during the load wait ends it at once with `decision.cancelled` and no send.
    #[test]
    fn decide_waits_for_the_local_model_to_load_before_sending() {
        let dir = config_dir(
            r#"{"local_runners":{"dec":{"kind":"mapika","host":"127.0.0.1","port":9100,"model":"jev-latest"}}}"#,
        );
        let transport = TimedTransport::new(vec![
            Ok(json_response(200, r#"{"ok":false}"#)),
            Ok(json_response(200, r#"{"ok":false}"#)),
            Ok(json_response(200, r#"{"ok":true,"model":"jev-latest"}"#)),
            Ok(ok_noul()),
        ]);
        let log = MemoryLog::new().expect("log");
        let ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
        let result = send(
            &options("dec", None, ctx, &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect("send");
        assert!(result.outcome.is_ok(), "{:?}", result.outcome);
        let sends = transport.sends();
        let paths: Vec<&str> = sends
            .iter()
            .map(|(_, sent)| sent.endpoint().as_str())
            .collect();
        assert_eq!(
            paths,
            ["http://127.0.0.1:9100/health"; 3]
                .into_iter()
                .chain(["http://127.0.0.1:9100/v1/systemone"])
                .collect::<Vec<_>>()
        );
        let (sent_at, sent) = &sends[3];
        assert!(*sent_at >= sends[2].0, "the send follows the loaded probe");
        assert_eq!(
            sent.deadline(),
            None,
            "no load time is added to the send's own timeout"
        );
        let load_wait = log
            .records()
            .expect("records")
            .into_iter()
            .filter(|record| record.action == "local.load_wait")
            .count();
        assert_eq!(load_wait, 4, "three probes and the end record");

        // Cancel while the probe blocks: the wait ends at once, nothing is sent.
        let blocking = BlockingTransport::new(Ok(json_response(200, r#"{"ok":false}"#)));
        let log = MemoryLog::new().expect("log");
        let ctx = context(dir.path(), &[], Arc::new(blocking.clone()), &log);
        let options = options("dec", None, ctx, &log);
        let cancel = CancelToken::new();
        let worker = {
            let cancel = cancel.clone();
            std::thread::spawn(move || {
                let mut results = Vec::new();
                let outcome = send_batch(
                    &options,
                    vec![request(NOUL), request(NOUL)],
                    &cancel,
                    &mut |result| {
                        results.push(result);
                        Ok(())
                    },
                );
                (outcome, results)
            })
        };
        blocking.wait_started(1);
        cancel.cancel();
        blocking.release();
        let (outcome, results) = worker.join().expect("worker");
        outcome.expect("batch");
        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|result| matches!(result.outcome, Err(DecisionError::Cancelled { sends: 0 })))
        );
        assert_eq!(
            blocking.sends(),
            1,
            "only the blocked probe; no decision send"
        );
    }

    /// AC: REQ-SAG-0015 (REQ-QA-005b). A model that is still not loaded after the runner's
    /// `load_timeout_secs` (10, the minimum) ends the send with `local.load_timeout` naming the
    /// runner, and no decision is sent. Real clock: `wait_until_loaded` (A-6) has no clock
    /// seam, so this waits the 10 s.
    #[test]
    fn a_model_that_does_not_load_in_time_is_local_load_timeout() {
        let dir = config_dir(
            r#"{"local_runners":{"dec":{"kind":"mapika","host":"127.0.0.1","port":9100,"model":"jev-latest","load_timeout_secs":10}}}"#,
        );
        let transport = TimedTransport::new(vec![Ok(json_response(200, r#"{"ok":false}"#))]);
        let log = MemoryLog::new().expect("log");
        let ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
        let started = std::time::Instant::now();
        let error = send(
            &options("dec", None, ctx, &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect_err("load timeout");
        assert!(started.elapsed() >= Duration::from_secs(10));
        assert_eq!(error.id(), "decide.decision");
        let backend = error.cause_detail().expect("cause");
        assert_eq!(backend.id, "decision.backend");
        let local = backend.cause.expect("local cause");
        assert_eq!(local.id, "local.load_timeout");
        assert!(
            local.message.contains("127.0.0.1:9100"),
            "{}",
            local.message
        );
        let sends = transport.sends();
        assert!(sends.len() >= 10, "one probe per second");
        assert!(
            sends
                .iter()
                .all(|(_, sent)| sent.endpoint().as_str() == "http://127.0.0.1:9100/health"),
            "no decision was sent"
        );
    }

    /// A load that never finishes within load_timeout is a command error; nothing is sent.
    #[test]
    fn a_runner_that_does_not_serve_the_model_is_a_command_error() {
        let dir = config_dir(
            r#"{"local_runners":{"dec":{"kind":"mapika","host":"127.0.0.1","port":9100,"model":"jev-latest"}}}"#,
        );
        let transport = ScriptedTransport::responses(vec![
            json_response(200, r#"{"ok":true,"model":"other"}"#),
            ok_noul(),
        ]);
        let log = MemoryLog::new().expect("log");
        let ctx = context(dir.path(), &[], Arc::new(transport.clone()), &log);
        let error = send(
            &options("dec", None, ctx, &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect_err("not served");
        assert_eq!(error.id(), "decide.decision");
        let cause = error.cause_detail().expect("cause");
        assert_eq!(cause.id, "decision.backend");
        assert_eq!(
            cause.cause.map(|local| local.id),
            Some("local.model_not_served".to_owned())
        );
        assert_eq!(transport.sends(), 1, "the probe only");
    }
}
