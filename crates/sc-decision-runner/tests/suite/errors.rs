//! REQ-SAG-0002: every lower error's id and message reach the top; panics are results;
//! a session-log failure never replaces the result.

use std::sync::Arc;

use sc_agent_http::{HttpError, HttpResponse, NetworkKind};
use sc_agent_types::{CancelToken, ErrorDetail, ErrorId};
use sc_decision_runner::RunnerError;
use sc_decision_schema::{Decision, DecisionError, SystemOneResponse};

use crate::support::*;

/// Every (id, message) of `detail` and its causes, top first.
fn levels(detail: ErrorDetail) -> Vec<(String, String)> {
    let mut levels = Vec::new();
    let mut next = Some(detail);
    while let Some(level) = next {
        levels.push((level.id.clone(), level.message.clone()));
        next = level.cause.map(|cause| *cause);
    }
    levels
}

/// A 200 reply whose response row is over the observability event cap.
fn oversize() -> HttpResponse {
    let blob = "x".repeat(sc_agent_log::MAX_ROW_BYTES + 1);
    let body = format!(
        r#"{{"model":"m","answers":{{"q":{{"type":"noul","noul":0.5}}}},"usage":{{"input_tokens":1,"output_tokens":1}},"blob":"{blob}"}}"#
    );
    HttpResponse::new(200, vec![], body.into_bytes())
}

/// REQ-SAG-0002: a transport failure, a backend decode failure and a session-log failure
/// each reach the returned error chain with their id and message.
#[test]
fn lower_errors_reach_the_top() {
    // Transport.
    let (transport, _dropped) = FnTransport::new(|_| {
        Err(
            HttpError::network(NetworkKind::Connect, 1, "could not connect").with_cause(
                ErrorDetail::new("io.ConnectionRefused", "connection refused", None),
            ),
        )
    });
    let harness = Harness::of(Arc::new(transport));
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    let error = result.outcome.expect_err("transport failed");
    let chain = levels(error.detail());
    assert_eq!(chain[0].0, "decision.http");
    assert_eq!(chain[1].0, "http.network");
    assert!(chain[1].1.contains("could not connect"), "{chain:?}");
    assert_eq!(
        chain[2],
        (
            "io.ConnectionRefused".to_owned(),
            "connection refused".to_owned()
        )
    );
    assert_eq!(
        error.to_string(),
        format!("decision.http: {}", error.message())
    );

    // Backend decode.
    let (transport, _dropped) = FnTransport::new(|_| Ok(ok()));
    let backend = ScriptedBackend::new().with_decode(|_, _, _, _| {
        Err(DecisionError::Backend {
            backend: "llm",
            detail: ErrorDetail::new(
                "llm.provider_refused",
                "the provider refused the prompt",
                Some(ErrorDetail::new("provider.code", "content_filter", None)),
            ),
        })
    });
    let harness = Harness::new(Arc::new(backend), Arc::new(transport), config(1), 5);
    let error = harness
        .runner
        .decide(request("m0"), &CancelToken::new())
        .outcome
        .expect_err("decode failed");
    let chain = levels(error.detail());
    assert_eq!(chain[0].0, "decision.backend");
    assert_eq!(
        chain[1],
        (
            "llm.provider_refused".to_owned(),
            "the provider refused the prompt".to_owned()
        )
    );
    assert_eq!(
        chain[2],
        ("provider.code".to_owned(), "content_filter".to_owned())
    );

    // Session log.
    let (transport, _dropped) = FnTransport::new(|_| Ok(oversize()));
    let harness = Harness::of(Arc::new(transport));
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    assert!(result.outcome.is_ok());
    let error = harness
        .runner
        .flush_session_log()
        .expect_err("a row was lost");
    assert!(matches!(error, RunnerError::SessionLog(_)));
    assert_eq!(error.id(), "runner.session_log");
    assert_eq!(
        error.to_string(),
        format!("runner.session_log: {}", error.message())
    );
    let chain = levels(error.detail());
    assert_eq!(chain[1].0, "log.rows_lost");
    assert!(
        chain.iter().any(|(id, _)| id == "log.row_too_large"),
        "the lost row's own error is in the chain: {chain:?}"
    );
    assert!(harness.runner.flush_session_log().is_ok(), "reported once");
}

/// REQ-SAG-0002: a backend panic is a decision.shutdown result with the payload text, and
/// the worker keeps working.
#[test]
fn panic_in_backend_is_a_result() {
    let (transport, _dropped) = FnTransport::new(|_| Ok(ok()));
    let backend = ScriptedBackend::new().with_decode(|index, response, _, _| {
        assert_ne!(index, 0, "decoder exploded");
        SystemOneResponse::from_json(response.status(), response.body())
    });
    let harness = Harness::new(Arc::new(backend), Arc::new(transport), config(1), 5);
    let mut results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(2).into_iter()), &CancelToken::new())
        .collect();
    results.sort_by_key(|result| result.position);
    match &results[0].outcome {
        Err(error @ DecisionError::Shutdown { reason }) => {
            assert_eq!(error.id(), "decision.shutdown");
            assert!(reason.contains("decoder exploded"), "{reason}");
        }
        other => panic!("{other:?}"),
    }
    assert!(results[1].outcome.is_ok(), "the worker survived the panic");
    let rows = harness.rows();
    assert_eq!(rows.len(), 2);
    let panicked = rows
        .iter()
        .find(|row| row.fields["position"] == 0)
        .expect("row of the panicked request");
    assert_eq!(panicked.fields["error"]["id"], "decision.shutdown");
    assert_eq!(
        panicked.fields["sends"], 1,
        "the send made before the panic is counted"
    );
}

/// REQ-SAG-0002: a session-log write failure never replaces the decision result; it is
/// logged as runner.session_log_failed with the request's session_id and position.
#[test]
fn session_log_failure_keeps_result() {
    let (transport, _dropped) = FnTransport::new(|_| Ok(oversize()));
    let harness = Harness::of(Arc::new(transport));
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    let response = result.outcome.as_ref().expect("the result is kept");
    assert!(response.extra.contains_key("blob"));
    assert!(harness.rows().is_empty(), "the oversize row was refused");
    let failed: Vec<_> = harness
        .records_of("sc_decision_runner")
        .into_iter()
        .filter(|record| record.action == "runner.session_log_failed")
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        failed[0].fields["session_id"],
        result.session_id.to_string()
    );
    assert_eq!(failed[0].fields["position"], 0);
    assert_eq!(failed[0].fields["error"]["id"], "log.row_too_large");
    assert!(harness.runner.flush_session_log().is_err());
}
