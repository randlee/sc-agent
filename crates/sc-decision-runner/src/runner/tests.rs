//! Runner tests that need the crate-private clock seam or job types.

use std::sync::atomic::AtomicU32;
use std::time::Duration;

use sc_agent_http::testing::{ManualClock, ScriptedTransport};
use sc_agent_http::{Endpoint, HttpResponse, Method, ResendPolicy};
use sc_agent_log::testing::MemoryLog;
use sc_agent_log::{Repo, SessionIdentity};
use sc_agent_types::{ErrorDetail, ErrorId};
use sc_decision_schema::SystemOneRequest;

use super::*;

/// Encodes the body as JSON; every decode is a schema mismatch, and the first one moves
/// the clock 91 s past the first send.
#[derive(Debug)]
struct NeverMatches {
    clock: ManualClock,
    decodes: AtomicU32,
}

impl Backend for NeverMatches {
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        let endpoint = Endpoint::parse("https://scripted.test/v1/systemone")?;
        Ok(HttpRequest::new(
            Method::Post,
            endpoint,
            request.body().to_json()?.into_bytes(),
        ))
    }

    fn decode(
        &self,
        _response: sc_agent_http::HttpResponse,
        _request: &DecisionRequest,
        _ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        if self.decodes.fetch_add(1, Ordering::AcqRel) == 0 {
            self.clock.advance(Duration::from_secs(91));
        }
        Err(DecisionError::SchemaMismatch {
            backend: "never-matches",
            detail: ErrorDetail::new("test.mismatch", "missing probabilities", None),
        })
    }
}

fn request() -> DecisionRequest {
    let body = SystemOneRequest::from_json(
        r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#,
    )
    .expect("body");
    DecisionRequest::try_from(body).expect("request")
}

/// REQ-SAG-0901 budget: the resend after a mismatch goes through `may_start`; when the
/// 90 s budget is used up the result is the kept `decision.schema_mismatch`, never
/// `decision.http` / `http.provider_busy`.
#[test]
fn budget_runs_out_after_mismatch() {
    let clock = ManualClock::new();
    let ok = HttpResponse::new(200, vec![], b"{}".to_vec());
    let transport = ScriptedTransport::responses(vec![ok.clone(), ok.clone(), ok]);
    let log = MemoryLog::new().expect("log");
    let http = HttpClient::new(
        Arc::new(transport.clone()),
        ResendPolicy::default(),
        log.general(),
    );
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "a-9", "test")
            .expect("identity");
    let backend = Arc::new(NeverMatches {
        clock: clock.clone(),
        decodes: AtomicU32::new(0),
    });
    let reading = clock.clone();
    let runner = DecisionRunner::with_clock(
        backend,
        http,
        log.session(identity),
        log.general(),
        RunnerConfig::default(),
        reading.clock(),
    );
    let result = runner.decide(request(), &CancelToken::new());
    let error = result.outcome.expect_err("no valid reply");
    assert_eq!(error.id(), "decision.schema_mismatch");
    assert!(matches!(error, DecisionError::SchemaMismatch { .. }));
    assert_eq!(transport.sends(), 1, "the budget refused the schema resend");
    let rows: Vec<_> = log
        .records()
        .expect("records")
        .into_iter()
        .filter(|record| record.action == SessionLogger::ACTION)
        .collect();
    assert_eq!(rows.len(), 1);
    let row = &rows[0].fields;
    assert_eq!(row["sends"], 1);
    assert_eq!(row["schema_resends"], 0);
    assert_eq!(row["error"]["id"], "decision.schema_mismatch");
}

/// A job's log context is its session id and position.
#[test]
fn job_log_context_is_session_and_position() {
    let session_id = SessionId::new();
    let request = request();
    let job = Job::Fresh(Box::new(Fresh {
        identity: Identity::new(session_id, 7, &request),
        request,
    }));
    assert_eq!(job.log_context(), LogContext::request(session_id, Some(7)));
    assert_eq!(
        job.identity().progress.tally(),
        crate::row::Tally::default()
    );
}

/// A-9 QA-002: the cancel hook is registered before the pool and feeder threads start. The
/// seam runs right after registration: no runner thread is alive yet, and a cancel there (a
/// token cancelled during the start) sends nothing and answers every request cancelled.
#[test]
fn the_cancel_hook_is_registered_before_any_thread_starts() {
    let ok = HttpResponse::new(200, vec![], b"{}".to_vec());
    let transport = ScriptedTransport::responses(vec![ok.clone(), ok.clone(), ok]);
    let log = MemoryLog::new().expect("log");
    let http = HttpClient::new(
        Arc::new(transport.clone()),
        ResendPolicy::default(),
        log.general(),
    );
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "a-9", "test")
            .expect("identity");
    let backend = Arc::new(NeverMatches {
        clock: ManualClock::new(),
        decodes: AtomicU32::new(0),
    });
    let runner = DecisionRunner::new(
        backend,
        http,
        log.session(identity),
        log.general(),
        RunnerConfig::default(),
    );
    let cancel = CancelToken::new();
    let seen = Arc::new(Mutex::new(None));
    {
        let (cancel, seen) = (cancel.clone(), Arc::clone(&seen));
        *runner.shared.after_hook.lock().expect("lock") = Some(Box::new(move |live| {
            *seen.lock().expect("lock") = Some(live);
            cancel.cancel();
        }));
    }
    let results: Vec<DecisionResult> = runner
        .decide_stream(Box::new((0..3).map(|_| request())), &cancel)
        .collect();
    assert_eq!(
        *seen.lock().expect("lock"),
        Some(0),
        "the hook is registered before any runner thread starts"
    );
    assert_eq!(results.len(), 3);
    assert!(
        results
            .iter()
            .all(|result| matches!(result.outcome, Err(DecisionError::Cancelled { .. })))
    );
    assert_eq!(transport.sends(), 0, "nothing is sent");
}

/// Encodes every request to one host (so one provider pause covers both) and decodes the
/// reply as a SystemOneResponse.
#[derive(Debug)]
struct OneHost;

impl Backend for OneHost {
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        let endpoint = Endpoint::parse("https://one-host.test/v1/systemone")?;
        Ok(HttpRequest::new(
            Method::Post,
            endpoint,
            request.body().to_json()?.into_bytes(),
        ))
    }

    fn decode(
        &self,
        response: sc_agent_http::HttpResponse,
        _request: &DecisionRequest,
        _ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        SystemOneResponse::from_json(response.status(), response.body())
    }
}

/// Records each send with the manual clock's reading; request `m0`'s first send is a 529
/// with `Retry-After: 1`, every other send a 200.
struct ClockedTransport {
    clock: ManualClock,
    sends: Mutex<Vec<(String, Instant)>>,
}

impl sc_agent_http::Transport for ClockedTransport {
    fn send(
        &self,
        request: &HttpRequest,
    ) -> Result<sc_agent_http::HttpResponse, sc_agent_http::HttpError> {
        let body: serde_json::Value = serde_json::from_slice(request.body()).expect("json");
        let model = body["model"].as_str().expect("model").to_owned();
        let mut sends = self.sends.lock().expect("lock");
        let first = !sends.iter().any(|(seen, _)| *seen == model);
        sends.push((model.clone(), self.clock.now()));
        Ok(if model == "m0" && first {
            HttpResponse::new(
                529,
                vec![("retry-after".to_owned(), "1".to_owned())],
                br#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#
                    .to_vec(),
            )
        } else {
            HttpResponse::new(
                200,
                vec![],
                br#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#
                    .to_vec(),
            )
        })
    }
}

/// QA-R2-002 (REQ-SAG-0012, runner side): A's 529 `Retry-After: 1` pauses the host on the
/// one shared HttpClient; B, admitted while the pause holds, is held until exactly A's pause
/// end. Deterministic: a ManualClock that only the test moves; one worker; the
/// crate-private `paused` seam reports B's refused admission before the clock moves, and the
/// clock then jumps to exactly the client's `not_before`. No real sleep and no real-time wait:
/// the held job parks on the manual clock and the advance wakes it.
#[test]
fn provider_pause_holds_other_jobs_until_exactly_its_end() {
    let clock = ManualClock::new();
    let transport = Arc::new(ClockedTransport {
        clock: clock.clone(),
        sends: Mutex::new(Vec::new()),
    });
    let log = MemoryLog::new().expect("log");
    let http = HttpClient::new(
        Arc::clone(&transport) as Arc<dyn sc_agent_http::Transport>,
        ResendPolicy::default(),
        log.general(),
    );
    let pause = http.clone();
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "a-9", "test")
            .expect("identity");
    let reading = clock.clone();
    let runner = DecisionRunner::with_clock(
        Arc::new(OneHost),
        http,
        log.session(identity),
        log.general(),
        RunnerConfig::new(1).expect("one worker"),
        reading.clock(),
    );
    let (paused_to, paused) = std::sync::mpsc::channel();
    *runner.shared.paused.lock().expect("lock") = Some(paused_to);
    let model = |name: &str| {
        let body = SystemOneRequest::from_json(&format!(
            r#"{{"model":"{name}","state":"s","questions":{{"q":{{"type":"noul"}}}}}}"#
        ))
        .expect("body");
        DecisionRequest::try_from(body).expect("request")
    };
    let runner = Arc::new(runner);
    let (results_to, results) = std::sync::mpsc::channel();
    let consumer = {
        let runner = Arc::clone(&runner);
        let requests = vec![model("m0"), model("m1")];
        std::thread::spawn(move || {
            for result in runner.decide_stream(Box::new(requests.into_iter()), &CancelToken::new())
            {
                let _ = results_to.send(result);
            }
        })
    };
    let failsafe = sc_agent_testkit::HANG_GUARD;
    let (position, at) = paused
        .recv_timeout(failsafe)
        .expect("B's admission is refused by A's pause");
    assert_eq!(position, 1, "B, not A, is refused admission");
    assert_eq!(
        pause.pause().not_before("one-host.test"),
        Some(at),
        "B is held until the client's not-before"
    );
    let busy_at = clock.now();
    assert!(
        at >= busy_at + Duration::from_secs(1),
        "Retry-After: 1 (plus jitter)"
    );
    assert_eq!(
        transport.sends.lock().expect("lock").len(),
        1,
        "only A's 529 so far: B waits while the clock stands still"
    );
    clock.advance(at - busy_at);
    let all: Vec<DecisionResult> = (0..2)
        .map(|_| results.recv_timeout(failsafe).expect("a result"))
        .collect();
    sc_agent_testkit::join_within(consumer, sc_agent_testkit::HANG_GUARD).expect("consumer");
    assert!(all.iter().all(|result| result.outcome.is_ok()), "{all:?}");
    let sends = transport.sends.lock().expect("lock");
    let b_first = sends
        .iter()
        .find(|(model, _)| model == "m1")
        .map(|(_, at)| *at)
        .expect("B's send");
    assert_eq!(b_first, at, "B's first send is at exactly A's pause end");
}

/// QA-004 (QA-R2-006): every runner thread is uncounted before the last producer closes the
/// result queue. The crate-private `live_at_close` seam reads `live_threads` inside that
/// `producer_done` itself, so the check does not depend on when the consumer wakes: with the
/// decrement after `producer_done` (in a worker's `Exit` or in the feeder's `FeedEnd`) the
/// closing producer still counts itself and the seam reads 1.
#[test]
fn every_runner_thread_is_uncounted_when_the_results_close() {
    let ok = || {
        HttpResponse::new(
            200,
            vec![],
            br#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#
                .to_vec(),
        )
    };
    for workers in [1, 2, 4] {
        let transport = ScriptedTransport::responses((0..6).map(|_| ok()).collect());
        let log = MemoryLog::new().expect("log");
        let http = HttpClient::new(Arc::new(transport), ResendPolicy::default(), log.general());
        let identity =
            SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "a-9", "test")
                .expect("identity");
        let runner = DecisionRunner::new(
            Arc::new(OneHost),
            http,
            log.session(identity),
            log.general(),
            RunnerConfig::new(workers).expect("workers"),
        );
        let results: Vec<DecisionResult> = runner
            .decide_stream(Box::new((0..6).map(|_| request())), &CancelToken::new())
            .collect();
        assert_eq!(results.len(), 6);
        assert!(results.iter().all(|result| result.outcome.is_ok()));
        let seen = runner.shared.live_at_close.lock().expect("lock").clone();
        assert_eq!(
            seen,
            vec![0],
            "{workers} workers: live threads when the results closed"
        );
    }
}

/// QA-004, the feeder's side (M10c): the feeder is forced to be the last producer. The
/// stream is abandoned while the feeder is blocked in the input iterator; the workers exit
/// (the test waits until only the feeder is left as a producer); then the input ends and the
/// feeder's `FeedEnd` closes the queue. The seam reads `live_threads` inside that last
/// `producer_done`: 0 when the feeder uncounts itself first.
#[test]
fn the_feeder_is_uncounted_when_it_closes_the_results() {
    struct Gated {
        left: Vec<DecisionRequest>,
        blocked: std::sync::mpsc::Sender<()>,
        open: std::sync::mpsc::Receiver<()>,
    }
    impl Iterator for Gated {
        type Item = DecisionRequest;
        fn next(&mut self) -> Option<DecisionRequest> {
            if self.left.is_empty() {
                let _ = self.blocked.send(());
                let _ = self.open.recv_timeout(sc_agent_testkit::HANG_GUARD);
                return None;
            }
            self.left.pop()
        }
    }
    let failsafe = sc_agent_testkit::HANG_GUARD;
    let ok = HttpResponse::new(
        200,
        vec![],
        br#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#
            .to_vec(),
    );
    let transport = ScriptedTransport::responses(vec![ok.clone(), ok]);
    let log = MemoryLog::new().expect("log");
    let http = HttpClient::new(Arc::new(transport), ResendPolicy::default(), log.general());
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "a-9", "test")
            .expect("identity");
    let runner = DecisionRunner::new(
        Arc::new(OneHost),
        http,
        log.session(identity),
        log.general(),
        RunnerConfig::new(2).expect("two workers"),
    );
    let (blocked_to, blocked) = std::sync::mpsc::channel();
    let (open, open_from) = std::sync::mpsc::channel();
    let input = Gated {
        left: vec![request(), request()],
        blocked: blocked_to,
        open: open_from,
    };
    let mut stream = runner.decide_stream(Box::new(input), &CancelToken::new());
    assert!(stream.next().is_some_and(|result| result.outcome.is_ok()));
    assert!(stream.next().is_some_and(|result| result.outcome.is_ok()));
    blocked
        .recv_timeout(failsafe)
        .expect("the feeder waits for more input");
    drop(stream);
    let results = runner
        .shared
        .last_results
        .lock()
        .expect("lock")
        .clone()
        .expect("the run's result queue");
    assert_eq!(
        results.wait_producers_at_most(1, failsafe),
        1,
        "both workers exited; the feeder is the last producer"
    );
    assert!(runner.shared.live_at_close.lock().expect("lock").is_empty());
    let _ = open.send(());
    assert_eq!(
        results.wait_producers_at_most(0, failsafe),
        0,
        "the feeder exited"
    );
    let seen = runner.shared.live_at_close.lock().expect("lock").clone();
    assert_eq!(
        seen,
        vec![0],
        "live threads when the feeder closed the results"
    );
}

/// QA-004 / QA-003 (round 3), the workers' side (M10a): a worker is forced to be the last
/// producer. Request 0's send is held in the transport until the feeder has exited (the test
/// waits until only the two workers are left as producers; the idle worker cannot exit while
/// request 0 runs). Then the send is released, both workers exit, and the closing
/// `producer_done` is a worker's. The seam must read 0 there.
#[test]
fn a_worker_is_uncounted_when_it_closes_the_results() {
    use std::sync::Condvar;

    let failsafe = sc_agent_testkit::HANG_GUARD;
    let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
    struct HeldFirst {
        gate: Arc<(Mutex<(bool, bool)>, Condvar)>,
        ok: HttpResponse,
    }
    impl sc_agent_http::Transport for HeldFirst {
        fn send(
            &self,
            request: &HttpRequest,
        ) -> Result<sc_agent_http::HttpResponse, sc_agent_http::HttpError> {
            let body: serde_json::Value = serde_json::from_slice(request.body()).expect("json");
            if body["model"] == "held" {
                let (lock, condvar) = &*self.gate;
                let mut state = lock.lock().expect("lock");
                state.0 = true;
                condvar.notify_all();
                let (state, timeout) = condvar
                    .wait_timeout_while(state, sc_agent_testkit::HANG_GUARD, |state| !state.1)
                    .expect("wait");
                assert!(!timeout.timed_out(), "never released");
                drop(state);
            }
            Ok(self.ok.clone())
        }
    }
    let ok = HttpResponse::new(
        200,
        vec![],
        br#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#
            .to_vec(),
    );
    let log = MemoryLog::new().expect("log");
    let http = HttpClient::new(
        Arc::new(HeldFirst {
            gate: Arc::clone(&gate),
            ok,
        }),
        ResendPolicy::default(),
        log.general(),
    );
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "a-9", "test")
            .expect("identity");
    let runner = DecisionRunner::new(
        Arc::new(OneHost),
        http,
        log.session(identity),
        log.general(),
        RunnerConfig::new(2).expect("two workers"),
    );
    let model = |name: &str| {
        let body = SystemOneRequest::from_json(&format!(
            r#"{{"model":"{name}","state":"s","questions":{{"q":{{"type":"noul"}}}}}}"#
        ))
        .expect("body");
        DecisionRequest::try_from(body).expect("request")
    };
    let stream = runner.decide_stream(
        Box::new(vec![model("held"), model("free")].into_iter()),
        &CancelToken::new(),
    );
    {
        let (lock, condvar) = &*gate;
        let (state, timeout) = condvar
            .wait_timeout_while(lock.lock().expect("lock"), failsafe, |state| !state.0)
            .expect("wait");
        assert!(!timeout.timed_out(), "the held send started");
        drop(state);
    }
    let results = runner
        .shared
        .last_results
        .lock()
        .expect("lock")
        .clone()
        .expect("the run's result queue");
    assert_eq!(
        results.wait_producers_at_most(2, failsafe),
        2,
        "the feeder exited; both workers are still producers"
    );
    assert!(runner.shared.live_at_close.lock().expect("lock").is_empty());
    {
        let (lock, condvar) = &*gate;
        lock.lock().expect("lock").1 = true;
        condvar.notify_all();
    }
    let all: Vec<DecisionResult> = stream.collect();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|result| result.outcome.is_ok()));
    assert_eq!(results.wait_producers_at_most(0, failsafe), 0);
    let seen = runner.shared.live_at_close.lock().expect("lock").clone();
    assert_eq!(
        seen,
        vec![0],
        "live threads when a worker closed the results"
    );
}
