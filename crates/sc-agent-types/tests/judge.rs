//! Judge contract (REQ-SAG-0103 re-frozen at r-1 merge, D10-break).

use std::collections::BTreeMap;
use std::sync::Arc;

use sc_agent_types::{
    Attestation, BackendAttestation, BackendCapabilities, BackendId, CanonicalJudgePayload,
    ChoiceId, EqualInputRecord, JevError, JudgeBackend, JudgeOutcome, JudgeRequest, LlmWireContext,
    MultiBackendMode, PayloadBundle, QuestionAnswer, QuestionId, Sha256Digest,
};
use serde_json::json;

fn bundle() -> PayloadBundle {
    let payload: CanonicalJudgePayload = serde_json::from_value(json!({
        "questions": [
            {"id": "relevant", "prompt": "Does it apply?", "choices": ["no", "yes"]},
            {"id": "risk", "prompt": "Is it risky?", "choices": ["high", "low"]}
        ],
        "state": [{"question_id": "relevant", "choice": "yes"}]
    }))
    .expect("payload");
    PayloadBundle::encode(&payload).expect("bundle")
}

fn request() -> JudgeRequest {
    JudgeRequest::new(Arc::new(bundle()), None)
}

fn choice(text: &str) -> ChoiceId {
    ChoiceId::try_from(text).expect("choice")
}

fn qid(text: &str) -> QuestionId {
    QuestionId::try_from(text).expect("id")
}

fn answers(relevant: &str, risk: &str) -> BTreeMap<QuestionId, QuestionAnswer> {
    BTreeMap::from([
        (
            qid("relevant"),
            QuestionAnswer::new(choice(relevant), None, None).expect("a"),
        ),
        (
            qid("risk"),
            QuestionAnswer::new(choice(risk), None, None).expect("b"),
        ),
    ])
}

/// Byte-identical serde for the three answer keys: a real a-10 Jev answer
/// (with `confidence`) and an LLM answer without it.
#[test]
fn test_answers_serde_golden() {
    let jev = r#"{"choice":"no","confidence":0.9,"probabilities":{"no":0.9,"yes":0.1}}"#;
    let parsed: QuestionAnswer = serde_json::from_str(jev).expect("jev answer");
    assert_eq!(parsed.confidence(), Some(0.9));
    assert_eq!(
        parsed.probabilities().expect("probabilities")[&choice("yes")],
        0.1
    );
    assert_eq!(serde_json::to_string(&parsed).expect("write"), jev);

    let llm = r#"{"choice":"yes"}"#;
    let parsed: QuestionAnswer = serde_json::from_str(llm).expect("llm answer");
    assert_eq!(parsed.confidence(), None);
    assert!(parsed.probabilities().is_none());
    assert_eq!(serde_json::to_string(&parsed).expect("write"), llm);

    let map: BTreeMap<QuestionId, QuestionAnswer> =
        serde_json::from_str(&format!(r#"{{"q":{jev},"r":{llm}}}"#)).expect("map");
    assert_eq!(
        serde_json::to_string(&map).expect("write"),
        format!(r#"{{"q":{jev},"r":{llm}}}"#)
    );
    assert_eq!(
        map[&qid("r")].to_choice_answer(&qid("r")),
        sc_agent_types::ChoiceAnswer {
            question_id: qid("r"),
            choice: choice("yes")
        }
    );
}

#[test]
fn test_question_answer_rejects_non_finite() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.5] {
        let error = QuestionAnswer::new(choice("yes"), Some(bad), None).expect_err("confidence");
        assert_eq!(error.code(), "RESPONSE_INVALID", "{bad}");
        let error = QuestionAnswer::new(
            choice("yes"),
            None,
            Some(BTreeMap::from([(choice("yes"), bad)])),
        )
        .expect_err("probability");
        assert_eq!(error.code(), "RESPONSE_INVALID", "{bad}");
    }
    for text in [
        r#"{"choice":"yes","confidence":1.5}"#,
        r#"{"choice":"yes","probabilities":{"yes":-1}}"#,
    ] {
        assert!(
            serde_json::from_str::<QuestionAnswer>(text).is_err(),
            "{text}"
        );
    }
    assert!(QuestionAnswer::new(choice("yes"), Some(0.0), None).is_ok());
    assert!(QuestionAnswer::new(choice("yes"), Some(1.0), None).is_ok());
}

#[test]
fn test_outcome_rejects_mismatched_answer_keys() {
    let request = request();
    let mut missing = answers("yes", "low");
    missing.remove(&qid("risk"));
    let error = JudgeOutcome::new(BackendId::Jev, &request, missing).expect_err("missing");
    assert_eq!(error.code(), "RESPONSE_INVALID");
    let mut extra = answers("yes", "low");
    extra.insert(
        qid("other"),
        QuestionAnswer::new(choice("yes"), None, None).expect("x"),
    );
    let error = JudgeOutcome::new(BackendId::Jev, &request, extra).expect_err("extra");
    assert_eq!(error.code(), "RESPONSE_INVALID");
    let ok = JudgeOutcome::new(BackendId::Jev, &request, answers("yes", "low")).expect("ok");
    assert_eq!(ok.answers().len(), 2);
    assert!(ok.input_sha256().is_none());
}

#[test]
fn test_outcome_rejects_choice_not_offered() {
    let request = request();
    let error =
        JudgeOutcome::new(BackendId::Jev, &request, answers("maybe", "low")).expect_err("choice");
    assert_eq!(error.code(), "RESPONSE_INVALID");
    let mut with_probabilities = answers("yes", "low");
    with_probabilities.insert(
        qid("relevant"),
        QuestionAnswer::new(
            choice("yes"),
            Some(0.8),
            Some(BTreeMap::from([
                (choice("yes"), 0.8),
                (choice("perhaps"), 0.2),
            ])),
        )
        .expect("answer"),
    );
    let error = JudgeOutcome::new(BackendId::Jev, &request, with_probabilities)
        .expect_err("probability key");
    assert_eq!(error.code(), "RESPONSE_INVALID");
}

#[test]
fn test_equal_input_equal_is_derived() {
    let digest = bundle().sha256().clone();
    let matched = EqualInputRecord::new(
        digest.clone(),
        MultiBackendMode::Independent,
        vec![
            BackendAttestation::new(BackendId::Jev, Attestation::Matched),
            BackendAttestation::new(BackendId::AnthropicHaiku, Attestation::Matched),
        ],
        false,
        None,
    );
    assert!(matched.equal());
    let value = serde_json::to_value(&matched).expect("json");
    assert_eq!(value["equal"], true);
    assert_eq!(value["mode"], "independent");
    assert_eq!(
        serde_json::from_value::<EqualInputRecord>(value.clone()).expect("read"),
        matched
    );

    let mut forged = value;
    forged["equal"] = json!(false);
    assert!(serde_json::from_value::<EqualInputRecord>(forged).is_err());

    for other in [
        Attestation::Mismatched {
            reported: Sha256Digest::of(b"other"),
        },
        Attestation::Unattested,
        Attestation::Failed,
        Attestation::NotSent,
    ] {
        let record = EqualInputRecord::new(
            digest.clone(),
            MultiBackendMode::ChainContext,
            vec![
                BackendAttestation::new(BackendId::Jev, Attestation::Matched),
                BackendAttestation::new(BackendId::OpenAiLuna, other.clone()),
            ],
            false,
            None,
        );
        assert!(!record.equal(), "{other:?}");
        let value = serde_json::to_value(&record).expect("json");
        assert_eq!(value["equal"], false);
        let back: EqualInputRecord = serde_json::from_value(value).expect("read");
        assert_eq!(back, record);
    }
    let empty = EqualInputRecord::new(digest, MultiBackendMode::default(), Vec::new(), false, None);
    assert!(!empty.equal());
    assert_eq!(MultiBackendMode::default(), MultiBackendMode::Independent);
}

#[test]
fn test_payload_bundle_encode_and_question_choices() {
    let bundle = bundle();
    assert_eq!(bundle.sha256(), &Sha256Digest::of(bundle.bytes()));
    let ids: Vec<&str> = bundle.question_ids().map(QuestionId::as_str).collect();
    assert_eq!(ids, ["relevant", "risk"]);
    assert!(bundle.question_choices()[&qid("risk")].contains(&choice("high")));
    assert_eq!(
        PayloadBundle::decode(bundle.bytes()).expect("decode"),
        bundle
    );
    let duplicate: CanonicalJudgePayload = serde_json::from_value(json!({
        "questions": [
            {"id": "q", "prompt": "a", "choices": ["x"]},
            {"id": "q", "prompt": "b", "choices": ["y"]}
        ],
        "state": []
    }))
    .expect("payload");
    assert_eq!(
        PayloadBundle::encode(&duplicate).expect_err("dup").code(),
        "INPUT_INVALID"
    );
}

/// `capabilities()` is a provided method that reads the descriptor; an
/// implementor restates nothing.
#[test]
fn test_capabilities_come_from_the_descriptor() {
    struct Minimal;

    impl JudgeBackend for Minimal {
        fn id(&self) -> BackendId {
            BackendId::OpenAiLuna
        }

        fn validate_for_send(&self, _req: &JudgeRequest) -> Result<(), JevError> {
            Ok(())
        }

        fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
            JudgeOutcome::new(self.id(), req, BTreeMap::new())
        }
    }

    let caps = Minimal.capabilities();
    assert_eq!(
        caps,
        BackendCapabilities::from_descriptor(BackendId::OpenAiLuna.descriptor())
    );
    assert!(caps.requires_llm_context());
    assert!(caps.supports_session_ask());
    assert_eq!(caps.request_limit_bytes(), 96_000);
    let request = JudgeRequest::new(Arc::new(bundle()), Some(LlmWireContext::new("p")));
    assert!(request.llm().is_some());
    assert!(!request.cancel().is_cancelled());
    let cancel = sc_agent_types::CancelToken::new();
    let request = request.with_cancel(cancel.clone());
    cancel.cancel();
    assert!(request.cancel().is_cancelled());
}
