//! decide_stream signature (PLAN-CRIT-001) and ADR-SAG-0001 (one Decision over any Backend).

use std::sync::{Arc, Mutex};

use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, LogContext, Method};
use sc_agent_types::CancelToken;
use sc_decision_runner::{DecisionRunner, RunnerConfig};
use sc_decision_schema::{Backend, Decision, DecisionError, DecisionRequest, SystemOneResponse};

use crate::support::*;

/// decide_stream signature: an owned `'static` iterator passes through `&dyn Decision`.
fn run_through(decision: &dyn Decision, requests: Vec<DecisionRequest>) -> usize {
    decision
        .decide_stream(Box::new(requests.into_iter()), &CancelToken::new())
        .count()
}

/// decide_stream signature: `Box::new(vec.into_iter())` through `&dyn Decision`.
#[test]
fn static_iterator_accepted() {
    let (transport, _dropped) = FnTransport::new(|_| Ok(ok()));
    let harness = Harness::of(Arc::new(transport));
    assert_eq!(run_through(as_decision(&harness.runner), requests(2)), 2);
}

/// A second, different backend: GET-less form-style path and its own decode.
#[derive(Debug)]
struct PathBackend;

impl Backend for PathBackend {
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        let endpoint = Endpoint::parse("https://other.test/decide")?;
        Ok(HttpRequest::new(
            Method::Post,
            endpoint,
            request.body().to_json()?.into_bytes(),
        )
        .with_header("x-backend", "path")?)
    }

    fn decode(
        &self,
        response: HttpResponse,
        _request: &DecisionRequest,
        _ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        let mut decided = SystemOneResponse::from_json(response.status(), response.body())?;
        decided.model = "path-backend".to_owned();
        Ok(decided)
    }
}

/// ADR-SAG-0001: the same DecisionRunner type drives two different Backend
/// implementations, each held as `Arc<dyn Backend>`.
#[test]
fn two_backends_one_runner() {
    let endpoints = Arc::new(Mutex::new(Vec::new()));
    let backends: [Arc<dyn Backend>; 2] = [Arc::new(ScriptedBackend::new()), Arc::new(PathBackend)];
    let mut models = Vec::new();
    for backend in backends {
        let seen = Arc::clone(&endpoints);
        let (transport, _dropped) = FnTransport::new(move |request| {
            lock(&seen).push(request.endpoint().as_str().to_owned());
            Ok(ok())
        });
        let harness = Harness::new(backend, Arc::new(transport), RunnerConfig::default(), 5);
        let runner: &DecisionRunner = &harness.runner;
        let result = runner.decide(request("m0"), &CancelToken::new());
        models.push(result.outcome.expect("decided").model);
    }
    assert_eq!(models, ["m", "path-backend"]);
    assert_eq!(
        *lock(&endpoints),
        [
            "https://scripted.test/v1/systemone",
            "https://other.test/decide"
        ]
    );
}
