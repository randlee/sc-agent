//! Dispatch, batch output and cancel: fake `Decision`s for the CRIT-M3 and ordering checks,
//! the real runner for the rest.

mod common;

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    use decide::input::{build_request, read_batch, request_bytes};
    use decide::output::result_line;
    use decide::{DecideError, send, send_batch, send_batch_on, send_on};
    use sc_agent_http::testing::{BlockingTransport, ScriptedTransport};
    use sc_agent_http::{HttpError, NetworkKind};
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_types::{CancelToken, ErrorId, SessionId};
    use sc_decision_schema::{
        Decision, DecisionError, DecisionRequest, DecisionResult, SystemOneResponse,
    };
    use serde_json::Value;

    use crate::common::{
        NOUL, NOUL_RESPONSE, TYPESAFE_KEY, config_dir, context, empty_config_dir, json_response,
        ok_noul, options, request,
    };

    /// A fake `Decision` that counts calls, keeps the requests it got, and answers in the
    /// order `order` (positions), failing the positions in `fail`.
    #[derive(Default)]
    struct Fake {
        decide_calls: AtomicUsize,
        stream_calls: AtomicUsize,
        seen: Mutex<Vec<DecisionRequest>>,
        order: Vec<u64>,
        fail: Vec<u64>,
    }

    impl Fake {
        fn result(&self, request: &DecisionRequest, position: u64) -> DecisionResult {
            let outcome = if self.fail.contains(&position) {
                Err(DecisionError::Http(HttpError::network(
                    NetworkKind::Connect,
                    1,
                    "connection refused",
                )))
            } else {
                Ok(SystemOneResponse::from_json(200, NOUL_RESPONSE.as_bytes()).expect("response"))
            };
            DecisionResult::new(
                SessionId::new(),
                position,
                request.digests().clone(),
                outcome,
            )
        }
    }

    impl Decision for Fake {
        fn decide(&self, request: DecisionRequest, _cancel: &CancelToken) -> DecisionResult {
            self.decide_calls.fetch_add(1, Ordering::SeqCst);
            let result = self.result(&request, 0);
            self.seen.lock().expect("lock").push(request);
            result
        }

        fn decide_stream<'a>(
            &'a self,
            requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>,
            _cancel: &CancelToken,
        ) -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a> {
            self.stream_calls.fetch_add(1, Ordering::SeqCst);
            let requests: Vec<DecisionRequest> = requests.collect();
            let order = if self.order.is_empty() {
                (0..requests.len() as u64).collect()
            } else {
                self.order.clone()
            };
            let results: Vec<DecisionResult> = order
                .iter()
                .map(|&position| self.result(&requests[position as usize], position))
                .collect();
            self.seen.lock().expect("lock").extend(requests);
            Box::new(results.into_iter())
        }
    }

    /// AC: decide send. A request from a file and from stdin reaches `send_on` unchanged;
    /// one result line with `session_id` and `position`.
    #[test]
    fn a_request_from_a_file_or_stdin_reaches_the_decision_unchanged() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("request.json");
        std::fs::write(&file, NOUL).expect("write");
        let from_file =
            request_bytes(file.to_str().expect("utf-8"), &mut std::io::empty()).expect("file");
        let from_stdin = request_bytes("-", &mut Cursor::new(NOUL.as_bytes())).expect("stdin");
        for bytes in [from_file, from_stdin] {
            let fake = Fake::default();
            let built = build_request(&bytes, &[], false, None).expect("request");
            let result = send_on(&fake, built, &CancelToken::new());
            let seen = fake.seen.lock().expect("lock");
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].body().to_json().expect("json"), NOUL);
            let line: Value =
                serde_json::from_slice(&result_line(&result).expect("line")).expect("json");
            assert_eq!(line["position"], 0);
            assert!(line["session_id"].as_str().is_some_and(|id| id.len() == 26));
            assert!(line["digests"]["q"].is_string());
        }
        let missing =
            request_bytes("no/such/file.json", &mut std::io::empty()).expect_err("missing");
        assert_eq!(missing.id(), "decide.input_not_found");
    }

    /// AC: CRIT-M3 dispatch. Plain send calls `decide`; a one-line batch calls `decide_stream`.
    #[test]
    fn plain_send_calls_decide_and_a_batch_calls_decide_stream() {
        let fake = Fake::default();
        let _ = send_on(&fake, request(NOUL), &CancelToken::new());
        assert_eq!(fake.decide_calls.load(Ordering::SeqCst), 1);
        assert_eq!(fake.stream_calls.load(Ordering::SeqCst), 0);

        let fake = Fake::default();
        let batch = read_batch(&mut Cursor::new(format!("{NOUL}\n"))).expect("batch");
        assert_eq!(batch.len(), 1);
        send_batch_on(&fake, batch, &CancelToken::new(), &mut |_| Ok(())).expect("batch");
        assert_eq!(fake.decide_calls.load(Ordering::SeqCst), 0);
        assert_eq!(fake.stream_calls.load(Ordering::SeqCst), 1);
    }

    /// AC: REQ-SAG-0701 output. Results print as they finish (out of input order), each with
    /// its position; a failure is data with its ErrorDetail chain and the batch continues.
    #[test]
    fn batch_results_print_fastest_first_and_a_failure_does_not_stop_the_batch() {
        let fake = Fake {
            order: vec![2, 0, 1],
            fail: vec![0],
            ..Fake::default()
        };
        let batch =
            read_batch(&mut Cursor::new(format!("{NOUL}\n{NOUL}\n{NOUL}\n"))).expect("batch");
        let mut lines = Vec::new();
        send_batch_on(&fake, batch, &CancelToken::new(), &mut |result| {
            lines.push(serde_json::from_slice::<Value>(&result_line(&result)?).expect("json"));
            Ok(())
        })
        .expect("batch");
        let positions: Vec<u64> = lines
            .iter()
            .map(|line| line["position"].as_u64().expect("position"))
            .collect();
        assert_eq!(positions, vec![2, 0, 1]);
        let failed = &lines[1];
        assert!(failed.get("response").is_none());
        assert_eq!(failed["error"]["id"], "decision.http");
        assert_eq!(failed["error"]["cause"]["id"], "http.network");
        assert!(
            failed["error"]["cause"]["message"]
                .as_str()
                .is_some_and(|text| text.contains("connection refused"))
        );
        assert!(lines[0]["response"].is_object());
        assert!(lines[2]["response"].is_object());
    }

    /// AC: REQ-SAG-0002, REQ-SAG-0007. Through the real runner: a probability prints unchanged,
    /// and a transport failure prints `decision.http` -> `http.network` with both messages.
    #[test]
    fn numbers_print_unchanged_and_the_lowest_error_is_printed() {
        let transport = ScriptedTransport::new(vec![
            Ok(ok_noul()),
            Err(HttpError::network(
                NetworkKind::Connect,
                1,
                "connection refused by test",
            )),
        ]);
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        let ok = send(&options, request(NOUL), &CancelToken::new()).expect("send");
        let text = String::from_utf8(result_line(&ok).expect("line")).expect("utf-8");
        assert!(text.contains("0.123456789"), "{text}");

        let failed = send(&options, request(NOUL), &CancelToken::new()).expect("send");
        let line: Value =
            serde_json::from_slice(&result_line(&failed).expect("line")).expect("json");
        assert_eq!(line["error"]["id"], "decision.http");
        assert!(line["error"]["message"].as_str().is_some());
        assert_eq!(line["error"]["cause"]["id"], "http.network");
        assert!(
            line["error"]["cause"]["message"]
                .as_str()
                .is_some_and(|text| text.contains("connection refused by test"))
        );
    }

    /// AC: REQ-SAG-0701 output. A stdout write failure cancels the batch and returns
    /// `decide.write`; the rest is drained, and the stream still ends.
    #[test]
    fn a_failed_write_cancels_the_batch_and_returns_decide_write() {
        let transport = BlockingTransport::new(Ok(ok_noul()));
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport.clone()),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        let requests: Vec<DecisionRequest> = (0..20).map(|_| request(NOUL)).collect();
        let cancel = CancelToken::new();
        let releaser = {
            let transport = transport.clone();
            std::thread::spawn(move || {
                transport.wait_started(1);
                transport.release();
            })
        };
        let mut calls = 0;
        let error = send_batch(&options, requests, &cancel, &mut |_| {
            calls += 1;
            Err(DecideError::Write {
                source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe"),
            })
        })
        .expect_err("write failed");
        releaser.join().expect("releaser");
        assert_eq!(error.id(), "decide.write");
        assert!(cancel.is_cancelled(), "the token is cancelled");
        assert_eq!(calls, 1, "no result is handed out after the failure");
    }

    /// AC: REQ-SAG-0104 AC2. Cancelling a send whose transport is blocked returns
    /// `decision.cancelled` within the 2 s bound; releasing the transport adds no result.
    #[test]
    fn cancelling_a_blocked_send_returns_cancelled_at_once() {
        let transport = BlockingTransport::new(Ok(ok_noul()));
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport.clone()),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        let cancel = CancelToken::new();
        let (sender, results) = mpsc::channel();
        let worker = {
            let cancel = cancel.clone();
            std::thread::spawn(move || {
                let result = send(&options, request(NOUL), &cancel);
                let _ = sender.send(result);
            })
        };
        transport.wait_started(1);
        cancel.cancel();
        let result = results
            .recv_timeout(Duration::from_secs(2))
            .expect("a result within 2 s")
            .expect("send");
        assert!(matches!(
            result.outcome,
            Err(DecisionError::Cancelled { .. })
        ));
        transport.release();
        worker.join().expect("worker");
        assert!(
            results.recv_timeout(Duration::from_millis(200)).is_err(),
            "no later result"
        );
    }

    /// AC: REQ-SAG-0701 parallel. 40 requests over a BlockingTransport: 8 sends in flight with
    /// the default workers, 1 with `workers: 1`.
    #[test]
    fn a_batch_runs_workers_sends_in_parallel() {
        for (config, workers) in [(None, 8_usize), (Some(r#"{"workers":1}"#), 1)] {
            let transport = BlockingTransport::new(Ok(ok_noul()));
            let log = MemoryLog::new().expect("log");
            let dir = config.map_or_else(empty_config_dir, config_dir);
            let ctx = context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            );
            let options = options("jev", None, ctx, &log);
            let requests: Vec<DecisionRequest> = (0..40).map(|_| request(NOUL)).collect();
            let observer = {
                let transport = transport.clone();
                std::thread::spawn(move || {
                    transport.wait_started(workers);
                    // Every worker is blocked in a send, so no further send can start.
                    let in_flight = transport.sends();
                    transport.release();
                    in_flight
                })
            };
            let mut count = 0;
            send_batch(&options, requests, &CancelToken::new(), &mut |result| {
                assert!(result.outcome.is_ok());
                count += 1;
                Ok(())
            })
            .expect("batch");
            assert_eq!(observer.join().expect("observer"), workers);
            assert_eq!(count, 40);
            assert_eq!(transport.sends(), 40);
        }
    }

    /// AC: REQ-SAG-0504. Line 412 of a 1,000-line batch has a score question without
    /// `criteria`: decide stops with `decide.request_invalid` naming line 412, the field and
    /// the reason, and the transport is never called.
    #[test]
    fn a_bad_line_stops_the_batch_before_any_send() {
        let bad = r#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"score"}}}"#;
        let text: String = (1..=1000)
            .map(|line| {
                if line == 412 {
                    format!("{bad}\n")
                } else {
                    format!("{NOUL}\n")
                }
            })
            .collect();
        let error = read_batch(&mut Cursor::new(text)).expect_err("line 412");
        assert_eq!(error.id(), "decide.request_invalid");
        assert!(matches!(
            error,
            DecideError::RequestInvalid {
                line: Some(412),
                ..
            }
        ));
        let message = error.message();
        assert!(message.contains("line 412"), "{message}");
        assert!(message.contains("criteria"), "{message}");
        assert!(message.contains("questions.q"), "{message}");
        assert_eq!(
            error.cause_detail().map(|cause| cause.id),
            Some("decision.request_parse".to_owned())
        );
        // The CLI flow: the batch is read whole first, so the run's transport is never called.
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
        let text: String = (1..=1000)
            .map(|line| {
                if line == 412 {
                    format!("{bad}\n")
                } else {
                    format!("{NOUL}\n")
                }
            })
            .collect();
        let outcome = read_batch(&mut Cursor::new(text)).and_then(|requests| {
            send_batch(&options, requests, &CancelToken::new(), &mut |_| Ok(()))
        });
        assert!(outcome.is_err());
        assert_eq!(transport.sends(), 0);
    }

    /// AC: decide send, owner-local-decision-crate. A LocalBackend and a SystemOneBackend run
    /// batches in parallel through two real runners.
    #[test]
    fn local_and_typesafe_batches_run_in_parallel() {
        let local_dir = config_dir(
            r#"{"local_runners":{"laya":{"kind":"generic","host":"127.0.0.1","port":9000,"model":"jev-latest"}}}"#,
        );
        let models =
            r#"{"models":[{"name":"jev-latest","description":"d","release_date":"2026-09-15"}]}"#;
        let local = ScriptedTransport::new(
            std::iter::once(Ok(json_response(200, models)))
                .chain((0..5).map(|_| Ok(ok_noul())))
                .collect(),
        );
        let typesafe = ScriptedTransport::new((0..5).map(|_| Ok(ok_noul())).collect());
        let local_log = MemoryLog::new().expect("log");
        let typesafe_log = MemoryLog::new().expect("log");
        let local_options = options(
            "laya",
            None,
            context(local_dir.path(), &[], Arc::new(local.clone()), &local_log),
            &local_log,
        );
        let typesafe_dir = empty_config_dir();
        let typesafe_options = options(
            "jev",
            None,
            context(
                typesafe_dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(typesafe.clone()),
                &typesafe_log,
            ),
            &typesafe_log,
        );
        let run = |options: decide::SendOptions| {
            std::thread::spawn(move || {
                let mut ok = 0;
                send_batch(
                    &options,
                    (0..5).map(|_| request(NOUL)).collect(),
                    &CancelToken::new(),
                    &mut |result| {
                        ok += usize::from(result.outcome.is_ok());
                        Ok(())
                    },
                )
                .map(|()| ok)
            })
        };
        let local_run = run(local_options);
        let typesafe_run = run(typesafe_options);
        assert_eq!(local_run.join().expect("local").expect("batch"), 5);
        assert_eq!(typesafe_run.join().expect("typesafe").expect("batch"), 5);
        let local_sends = local.requests();
        assert_eq!(
            local_sends[0].endpoint().as_str(),
            "http://127.0.0.1:9000/v1/models",
            "load probe first"
        );
        assert!(
            local_sends[1..]
                .iter()
                .all(|sent| sent.endpoint().as_str() == "http://127.0.0.1:9000/v1/systemone")
        );
        assert!(
            typesafe
                .requests()
                .iter()
                .all(|sent| sent.endpoint().as_str() == "https://api.typesafe.ai/v1/systemone")
        );
    }

    /// AC: REQ-SAG-0701 output. decide's source starts no thread (the ctrlc handler thread is
    /// ctrlc's own).
    #[test]
    fn decide_source_spawns_no_thread() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&src).expect("src") {
            let path = entry.expect("entry").path();
            let text = std::fs::read_to_string(&path).expect("read");
            for needle in ["thread::spawn", "thread::Builder", "thread::scope"] {
                assert!(!text.contains(needle), "{} uses {needle}", path.display());
            }
            for needle in ["thread::sleep", "time::sleep", "sleep_until"] {
                assert!(!text.contains(needle), "{} uses {needle}", path.display());
            }
        }
    }
}
