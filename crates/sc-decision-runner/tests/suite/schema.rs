//! REQ-SAG-0901 AC2 / AC3: schema-mismatch resend.

use std::sync::Arc;

use sc_agent_http::testing::ScriptedTransport;
use sc_agent_types::{CancelToken, ErrorDetail};
use sc_decision_schema::DecisionError;

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

fn oks(n: usize) -> Vec<sc_agent_http::HttpResponse> {
    (0..n).map(|_| ok()).collect()
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

// Holdoff and send-budget cases are driven deterministically in src/runner/tests.rs.
