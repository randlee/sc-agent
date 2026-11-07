//! Consumer digest for the a-11 branches fixture (README tighten #2).

use std::collections::BTreeMap;
use std::path::PathBuf;

use sc_agent_types::schema::subjects::SubjectsDocument;
use sc_agent_types::{
    BackendId, BackendSet, CanonicalJudgePayload, PayloadBundle, RepoRelativePath, RepoRoot, Rev,
};
use sc_jev_check::config::{CheckSpec, InputSpec};
use sc_jev_check::equal_input::producer_from_extractions;
use sc_jev_check::payload::{build_payload, parse_question_set};
use sc_jev_check::run::{CheckSource, RunConfig, prepare, run};

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
    assert_eq!(document.subjects().len(), 6);
    let joined = document
        .subjects()
        .iter()
        .map(|subject| subject.text())
        .collect::<Vec<_>>()
        .join("\n");
    let questions = parse_question_set(QUESTIONS).expect("questions");
    assert_eq!(questions.len(), 1);
    let questions: Vec<_> = questions
        .into_iter()
        .map(|question| {
            let prompt = question.prompt().replace("{branches}", &joined);
            question.with_prompt(prompt)
        })
        .collect();
    let expected = PayloadBundle::encode(&CanonicalJudgePayload::new(questions, Vec::new()))
        .expect("expected bundle");

    let temp = tempfile::tempdir().expect("temp");
    let dir = temp.path();
    std::fs::write(dir.join("subjects.json"), &bytes).expect("copy subjects");
    std::fs::write(dir.join("questions.json"), QUESTIONS).expect("questions file");
    let repo = RepoRoot::new(dir);
    let mut spec = CheckSpec::new(RepoRelativePath::try_from("questions.json").expect("path"));
    spec.inputs = BTreeMap::from([(
        "branches".into(),
        InputSpec::Subjects {
            path: RepoRelativePath::try_from("subjects.json").expect("path"),
        },
    )]);
    let built = build_payload(&spec, &repo, &Rev::worktree()).expect("payload");
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
    assert_eq!(producer.language, document.producer().language);
    assert_eq!(producer.query_sha256, document.producer().query_sha256);
    assert_eq!(producer.mode, "matches");

    let config = RunConfig::new(
        repo,
        std::sync::Arc::new(sc_agent_types::observability::v2::NoopSessionLog),
    )
    .backends(BackendSet::try_from(vec![BackendId::Jev]).expect("set"))
    .dry_run(true);
    let dry = run(prepare(CheckSource::Spec(spec), config).expect("prepare")).expect("dry-run");
    assert_eq!(dry.prompt_sha256(), expected.sha256());
    assert!(dry.session_id().is_none());
    assert!(dry.results().is_empty());
}
