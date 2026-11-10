//! REQ-SAG-0901 AC2 / AC3: schema-mismatch resend.

use std::sync::Arc;

use sc_agent_http::testing::ScriptedTransport;
use sc_agent_types::{CancelToken, ErrorDetail, ErrorId};
use sc_decision_schema::{Decision, DecisionError, SystemOneResponse};

use crate::support::*;

fn mismatch() -> DecisionError {
    DecisionError::SchemaMismatch {
        backend: "scripted",
        detail: ErrorDetail::new(
            "scripted.schema",
            "answers.q: missing field `probabilities`",
            None,
        ),
    }
}

/// A backend that never matches.
fn never_matches() -> ScriptedBackend {
    ScriptedBackend::new().with_decode(|_, _, _, _| Err(mismatch()))
}

fn oks(n: usize) -> Vec<sc_agent_http::HttpResponse> {
    (0..n).map(|_| ok()).collect()
}

/// REQ-SAG-0901 AC2: a reply missing probabilities, then a valid one: the valid answer
/// after one resend (2 sends, schema_resends 1 in the row).
#[test]
fn schema_mismatch_resent_once() {
    let transport = ScriptedTransport::responses(oks(3));
    let backend = Arc::new(ScriptedBackend::new().with_decode(|index, response, _, _| {
        if index == 0 {
            Err(mismatch())
        } else {
            SystemOneResponse::from_json(response.status(), response.body())
        }
    }));
    let harness = Harness::new(backend.clone(), Arc::new(transport.clone()), config(1), 5);
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    assert!(result.outcome.is_ok(), "{result:?}");
    assert_eq!(transport.sends(), 2);
    assert_eq!(backend.encodes(), 1);
    assert_eq!(backend.decodes(), 2);
    let rows = harness.rows();
    assert_eq!(rows[0].fields["sends"], 2);
    assert_eq!(rows[0].fields["schema_resends"], 1);
}

/// REQ-SAG-0901 AC2: schema resends stop at once on cancel.
#[test]
fn cancel_ends_schema_resends() {
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let transport = ScriptedTransport::responses(oks(5));
    let backend = ScriptedBackend::new().with_decode(move |_, _, _, _| {
        token.cancel();
        Err(mismatch())
    });
    let harness = Harness::new(Arc::new(backend), Arc::new(transport.clone()), config(1), 5);
    let result = harness.decide(request("m0"), &cancel);
    let result = result
        .recv_timeout(HANG_GUARD)
        .expect("the result (hang guard)");
    assert!(
        matches!(result.outcome, Err(DecisionError::Cancelled { sends: 1 })),
        "{result:?}"
    );
    assert_eq!(transport.sends(), 1, "no resend after cancel");
}

/// REQ-SAG-0901 AC3: a backend that never matches gives decision.schema_mismatch, with
/// what did not match, after max_sends sends (5 by default, 2 with max_sends = 2).
#[test]
fn schema_mismatch_after_max_sends() {
    for max_sends in [5_u32, 2] {
        let transport = ScriptedTransport::responses(oks(8));
        let harness = Harness::new(
            Arc::new(never_matches()),
            Arc::new(transport.clone()),
            config(1),
            max_sends,
        );
        let result = harness.runner.decide(request("m0"), &CancelToken::new());
        let error = result.outcome.expect_err("never matches");
        assert_eq!(error.id(), "decision.schema_mismatch");
        let cause = error.cause_detail().expect("what did not match");
        assert_eq!(cause.message, "answers.q: missing field `probabilities`");
        assert_eq!(
            transport.sends(),
            max_sends as usize,
            "max_sends {max_sends}"
        );
        let rows = harness.rows();
        assert_eq!(rows[0].fields["sends"], max_sends);
        assert_eq!(rows[0].fields["schema_resends"], max_sends - 1);
    }
}

/// REQ-SAG-0901 AC3: 529 twice, then replies that never match: 5 sends in total, not more.
#[test]
fn busy_twice_then_mismatch_stops_at_five() {
    let mut replies = vec![busy("0"), busy("0")];
    replies.extend(oks(6));
    let transport = ScriptedTransport::responses(replies);
    let harness = Harness::new(
        Arc::new(never_matches()),
        Arc::new(transport.clone()),
        config(1),
        5,
    );
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    let error = result.outcome.expect_err("never matches");
    assert_eq!(error.id(), "decision.schema_mismatch");
    assert_eq!(transport.sends(), 5);
    let rows = harness.rows();
    assert_eq!(rows[0].fields["sends"], 5);
    assert_eq!(rows[0].fields["schema_resends"], 2);
}
