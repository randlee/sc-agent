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
    assert_eq!(job.ticket().sends, 0);
}
