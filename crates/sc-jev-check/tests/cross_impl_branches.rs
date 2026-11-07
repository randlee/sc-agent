//! Consumer digest for the a-11 branches fixture (README tighten #2).

use std::collections::BTreeMap;
use std::path::PathBuf;

use sc_agent_types::{
    BackendId, CanonicalJudgePayload, InputSpec, RepoRoot, Rev, SubjectsDocument,
    payload_bundle_from_value,
};
use sc_jev_check::config::CheckSpec;
use sc_jev_check::equal_input::producer_from_extractions;
use sc_jev_check::payload::{PayloadBuilder, parse_question_set};
use sc_jev_check::run::{RunOptions, run_check};

const QUESTIONS: &str = r#"{
  "schema": "plan-anchoring/questions/1",
  "version": 1,
  "questions": {
    "branches": {
      "type": "choice",
      "instructions": "Which branch applies?\n\n{branches}",
      "criteria": {"no": "No.", "yes": "Yes."}
    }
  }
}"#;

#[test]
fn cross_impl_branches() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cross-impl/branches/subjects.json");
    let bytes = std::fs::read(&fixture).expect("subjects.json");
    let document: SubjectsDocument = serde_json::from_slice(&bytes).expect("subjects document");
    assert_eq!(document.subjects.len(), 6);
    let joined = document
        .subjects
        .iter()
        .map(|subject| subject.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut questions = parse_question_set(QUESTIONS).expect("questions");
    assert_eq!(questions.len(), 1);
    questions[0].prompt = questions[0].prompt.replace("{branches}", &joined);
    let expected = payload_bundle_from_value(
        &serde_json::to_value(CanonicalJudgePayload {
            questions,
            state: Vec::new(),
        })
        .expect("value"),
    )
    .expect("expected bundle");

    let temp = tempfile::tempdir().expect("temp");
    let dir = temp.path();
    std::fs::write(dir.join("subjects.json"), &bytes).expect("copy subjects");
    std::fs::write(dir.join("questions.json"), QUESTIONS).expect("questions file");
    let repo = RepoRoot::new(dir);
    let spec = CheckSpec {
        questions_path: "questions.json".into(),
        state_path: None,
        system_prompt_file: None,
        inputs: BTreeMap::from([(
            "branches".into(),
            InputSpec::Subjects {
                path: "subjects.json".into(),
            },
        )]),
    };
    let built = PayloadBuilder::packaged()
        .expect("builder")
        .build(&spec, &repo, &Rev::worktree())
        .expect("payload");
    let payload = std::str::from_utf8(built.bundle.bytes()).expect("utf-8");
    assert!(payload.contains("Transition.GIVE_BACK"));
    assert!(payload.contains("Transition.KEEP"));
    assert!(payload.contains("Transition.STOP"));
    assert_eq!(built.bundle.sha256(), expected.sha256());
    assert_eq!(built.bundle.bytes(), expected.bytes());

    let producer = producer_from_extractions(&built.extractions)
        .expect("one producer")
        .expect("producer");
    assert_eq!(producer.name, "sc-treesitter");
    assert_eq!(producer.language, document.producer.language);
    assert_eq!(producer.query_sha256, document.producer.query_sha256);
    assert_eq!(producer.mode, "matches");

    let dry = run_check(&spec, &[BackendId::Jev], RunOptions::dry_run_only(repo)).expect("dry-run");
    assert_eq!(dry.bundle.sha256(), expected.sha256());
    assert!(dry.session_id.is_none());
    assert!(dry.outcomes.is_empty());
}
