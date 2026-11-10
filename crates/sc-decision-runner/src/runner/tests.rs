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
    let clock = ManualClock::new(Instant::now());
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
        Arc::new(move || reading.now()),
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
        clock: ManualClock::new(Instant::now()),
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
/// clock then jumps to exactly the client's `not_before`. No real sleep; no wall-clock value
/// is asserted (the real waits only bound how long the worker naps between clock reads).
#[test]
fn provider_pause_holds_other_jobs_until_exactly_its_end() {
    let clock = ManualClock::new(Instant::now());
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
        Arc::new(move || reading.now()),
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
    let failsafe = Duration::from_secs(60);
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
    consumer.join().expect("consumer");
    assert!(all.iter().all(|result| result.outcome.is_ok()), "{all:?}");
    let sends = transport.sends.lock().expect("lock");
    let b_first = sends
        .iter()
        .find(|(model, _)| model == "m1")
        .map(|(_, at)| *at)
        .expect("B's send");
    assert_eq!(b_first, at, "B's first send is at exactly A's pause end");
}
