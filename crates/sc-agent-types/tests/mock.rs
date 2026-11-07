//! `test_util::MockJudgeBackend` contract (D10.11, ADR-SAG-0022 r-1).
//!
//! Needs `--features test-util` (`[[test]] required-features`, D0 outcome).

use std::collections::BTreeMap;
use std::sync::Arc;

use sc_agent_types::test_util::MockJudgeBackend;
use sc_agent_types::{
    BackendId, CanonicalJudgePayload, JevError, JudgeBackend, JudgeRequest, LlmWireContext,
    PayloadBundle, QuestionAnswer, QuestionId, Sha256Digest,
};

fn bundle() -> Arc<PayloadBundle> {
    let payload: CanonicalJudgePayload = serde_json::from_value(serde_json::json!({
        "questions": [{"id": "q", "prompt": "Is it?", "choices": ["no", "yes"]}],
        "state": []
    }))
    .expect("payload");
    Arc::new(PayloadBundle::encode(&payload).expect("bundle"))
}

fn answers(choice: &str) -> BTreeMap<QuestionId, QuestionAnswer> {
    serde_json::from_value(serde_json::json!({"q": {"choice": choice}})).expect("answers")
}

#[test]
fn test_mock_judge_backend_contract() {
    let payload = bundle();
    let bare = JudgeRequest::new(Arc::clone(&payload), None);
    let prompted = JudgeRequest::new(Arc::clone(&payload), Some(LlmWireContext::new("judge")));

    // Outcome built at judge time against the request; requests recorded;
    // validations counted; input digest is the received bundle's.
    let jev = Arc::new(MockJudgeBackend::new(BackendId::Jev, answers("yes")));
    let registered: Arc<dyn JudgeBackend> = Arc::clone(&jev) as Arc<dyn JudgeBackend>;
    registered.validate_for_send(&bare).expect("validate");
    let outcome = registered.judge(&bare).expect("judge");
    assert_eq!(outcome.backend(), BackendId::Jev);
    assert_eq!(outcome.answers()["q"].choice().as_str(), "yes");
    assert_eq!(outcome.input_sha256(), Some(payload.sha256()));
    assert_eq!(jev.validations(), 1);
    assert_eq!(jev.requests(), vec![bare.clone()]);

    // Answers that do not match the request's questions are refused.
    let wrong = MockJudgeBackend::new(BackendId::Jev, answers("maybe"));
    assert_eq!(
        wrong.judge(&bare).expect_err("choice").code(),
        "RESPONSE_INVALID"
    );

    // Provider meta and a reported digest override.
    let reported = Sha256Digest::of(b"something else");
    let meta = MockJudgeBackend::new(BackendId::Jev, answers("no"))
        .with_provider_meta(serde_json::json!({"model": "mock"}))
        .with_reported_digest(reported.clone());
    let outcome = meta.judge(&bare).expect("judge");
    assert_eq!(outcome.provider_meta().expect("meta")["model"], "mock");
    assert_eq!(outcome.input_sha256(), Some(&reported));

    // Failure after validation: sent (recorded) and failed.
    let failing = MockJudgeBackend::new(BackendId::Jev, answers("no")).with_failure(
        JevError::network(sc_agent_types::NetworkErrorKind::Busy, "busy"),
    );
    failing.validate_for_send(&bare).expect("validation passes");
    assert_eq!(
        failing.judge(&bare).expect_err("fails").code(),
        "NETWORK_ERROR"
    );
    assert_eq!(failing.requests().len(), 1);

    // Refusal: validate_for_send fails, nothing is recorded.
    let refusing = MockJudgeBackend::refusing(BackendId::Jev, JevError::input_too_large(4, 8));
    assert_eq!(
        refusing
            .validate_for_send(&bare)
            .expect_err("refused")
            .code(),
        "INPUT_TOO_LARGE"
    );
    assert!(refusing.judge(&bare).is_err());
    assert_eq!(refusing.validations(), 1);
    assert!(refusing.requests().is_empty());

    // LLM ids require LLM context (ADR-SAG-0022, CPR-302).
    let haiku = MockJudgeBackend::new(BackendId::AnthropicHaiku, answers("yes"));
    assert_eq!(
        haiku
            .validate_for_send(&bare)
            .expect_err("no context")
            .code(),
        "INPUT_INVALID"
    );
    assert_eq!(
        haiku.judge(&bare).expect_err("no context").code(),
        "INPUT_INVALID"
    );
    assert_eq!(haiku.validations(), 1);
    assert!(haiku.requests().is_empty());
    haiku.validate_for_send(&prompted).expect("with context");
    haiku.judge(&prompted).expect("with context");
    assert_eq!(haiku.validations(), 2);
    assert_eq!(haiku.requests().len(), 1);
}
