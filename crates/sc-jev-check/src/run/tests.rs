//! Engine unit tests (REQ-SAG-0601, REQ-SAG-0610). Multi-backend payload
//! sharing, equal-input, and the packaged prompt default live in
//! `tests/sessions_exact.rs`; the response contract in
//! `tests/run_response.rs`.

use super::*;
use crate::config::PACKAGED_SYSTEM_PROMPT;
use crate::extract::test_support::{self, copy_fixture};
use sc_agent_types::observability::v2::{LogRejected, NoopSessionLog, SessionLog};
use sc_agent_types::test_util::MockJudgeBackend;
use std::path::PathBuf;

const QUESTIONS: &str = r#"{
  "schema": "plan-anchoring/questions/1",
  "version": 1,
  "questions": {
    "relevant": {
      "type": "choice",
      "instructions": "Does it apply?",
      "criteria": {"no": "No.", "yes": "Yes."}
    }
  }
}"#;

struct JsonlSink {
    path: PathBuf,
}

impl SessionLog for JsonlSink {
    fn emit(&self, event: LogEvent) -> Result<(), LogRejected> {
        use std::io::Write;
        let line = serde_json::json!({
            "session_id": event.session_id().to_string(),
            "backend": event.backend().to_string(),
            "prompt_sha256": event.prompt_sha256().as_str(),
            "operation": event.operation().as_str(),
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .expect("jsonl");
        writeln!(file, "{line}").expect("write jsonl");
        Ok(())
    }
}

fn bare_repo() -> tempfile::TempDir {
    let dir = test_support::temp_dir("run");
    std::fs::write(dir.path().join("questions.json"), QUESTIONS).expect("questions");
    std::fs::write(
        dir.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n",
    )
    .expect("check");
    dir
}

fn judge(id: BackendId, choice: &str) -> Arc<MockJudgeBackend> {
    let answers = serde_json::from_value(serde_json::json!({"relevant": {"choice": choice}}))
        .expect("answers");
    Arc::new(MockJudgeBackend::new(id, answers))
}

fn registry_of(judges: &[&Arc<MockJudgeBackend>]) -> Arc<BackendRegistry> {
    let mut registry = BackendRegistry::new();
    for judge in judges {
        registry
            .register(Arc::clone(*judge) as Arc<dyn JudgeBackend>)
            .expect("register");
    }
    Arc::new(registry)
}

fn set(ids: &[BackendId]) -> BackendSet {
    BackendSet::try_from(ids.to_vec()).expect("backend set")
}

fn config(repo: &Path, sessions: &Path) -> RunConfig {
    RunConfig::new(RepoRoot::new(repo), Arc::new(NoopSessionLog)).sessions_dir(sessions)
}

fn check_file() -> CheckSource {
    CheckSource::CheckFile("check.toml".into())
}

fn system_prompt(request: &JudgeRequest) -> &str {
    &request.llm().expect("llm prompt").system_prompt
}

#[test]
fn dry_run_sends_nothing_and_needs_no_registry_or_store() {
    let repo = copy_fixture();
    let sessions = test_support::temp_dir("dry-sessions");
    let sessions_dir = sessions.path().join("never");
    let jev = judge(BackendId::Jev, "yes");
    let prepared = prepare(
        check_file(),
        config(repo.path(), &sessions_dir)
            .dry_run(true)
            .backends(set(&[BackendId::Jev, BackendId::AnthropicHaiku]))
            .registry(registry_of(&[&jev])),
    )
    .expect("prepare");
    let digest = prepared.prompt_sha256().clone();
    let response = run(prepared).expect("dry-run");
    assert!(response.dry_run());
    assert!(response.session_id().is_none());
    assert!(!response.session_saved());
    assert!(response.results().is_empty());
    assert_eq!(response.prompt_sha256(), &digest);
    assert!(
        response
            .equal_input()
            .backends()
            .iter()
            .all(|entry| entry.attestation() == &Attestation::NotSent)
    );
    assert!(!response.equal_input().equal());
    assert_eq!(response.limits().len(), 2);
    assert!(response.payload().expect("payload")["questions"].is_array());
    assert!(jev.requests().is_empty());
    assert_eq!(
        jev.validations(),
        0,
        "a dry run constructs and calls no backend"
    );
    assert!(
        !sessions_dir.exists(),
        "a dry run creates no session directory"
    );
}

#[test]
fn packaged_prompt_is_injected_and_override_does_not_change_digest() {
    let repo = bare_repo();
    let sessions = test_support::temp_dir("prompt-sessions");
    std::fs::write(repo.path().join("override.md"), "OVERRIDE-PROMPT-TOKEN").expect("override");
    let first_judge = judge(BackendId::AnthropicHaiku, "yes");
    let first = run(prepare(
        check_file(),
        config(repo.path(), sessions.path())
            .backends(set(&[BackendId::AnthropicHaiku]))
            .registry(registry_of(&[&first_judge])),
    )
    .expect("prepare"))
    .expect("packaged");
    assert_eq!(
        system_prompt(&first_judge.requests()[0]),
        PACKAGED_SYSTEM_PROMPT
    );
    let second_judge = judge(BackendId::AnthropicHaiku, "yes");
    let second = run(prepare(
        check_file(),
        config(repo.path(), sessions.path())
            .backends(set(&[BackendId::AnthropicHaiku]))
            .system_prompt_override(repo.path().join("override.md"))
            .registry(registry_of(&[&second_judge])),
    )
    .expect("prepare"))
    .expect("override");
    assert_eq!(
        system_prompt(&second_judge.requests()[0]),
        "OVERRIDE-PROMPT-TOKEN"
    );
    assert_eq!(first.prompt_sha256(), second.prompt_sha256());
}

#[test]
fn check_file_prompt_beats_packaged_asset() {
    let repo = copy_fixture();
    let sessions = test_support::temp_dir("check-prompt");
    let luna = judge(BackendId::OpenAiLuna, "yes");
    run(prepare(
        check_file(),
        config(repo.path(), sessions.path())
            .backends(set(&[BackendId::OpenAiLuna]))
            .registry(registry_of(&[&luna])),
    )
    .expect("prepare"))
    .expect("run");
    let requests = luna.requests();
    let prompt = system_prompt(&requests[0]);
    assert!(prompt.contains("PROMPT-TOKEN-a3-check"));
    assert!(!prompt.contains(PACKAGED_SYSTEM_PROMPT.trim()));
}

#[test]
fn missing_llm_prompt_fails_in_prepare_and_jev_ignores_it() {
    let repo = bare_repo();
    std::fs::write(
        repo.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n\n[llm_judge]\nsystem_prompt_file = \"missing.md\"\n",
    )
    .expect("check");
    let sessions = test_support::temp_dir("missing-prompt");
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let error = prepare(
        check_file(),
        config(repo.path(), sessions.path())
            .backends(set(&[BackendId::AnthropicHaiku]))
            .registry(registry_of(&[&haiku])),
    )
    .expect_err("missing prompt");
    assert_eq!(error.code(), "PROMPT_FILE_MISSING");
    assert!(haiku.requests().is_empty());
    assert_eq!(std::fs::read_dir(sessions.path()).expect("dir").count(), 0);
    let jev = run(prepare(
        check_file(),
        config(repo.path(), sessions.path())
            .registry(registry_of(&[&judge(BackendId::Jev, "yes")])),
    )
    .expect("jev ignores the missing prompt"))
    .expect("run");
    assert!(jev.primary().expect("primary").answers()["relevant"].choice() == "yes");
}

#[test]
fn refusing_backend_does_not_log_or_save() {
    let repo = bare_repo();
    let sessions = test_support::temp_dir("refuse-sessions");
    let log_dir = test_support::temp_dir("jsonl");
    let log_path = log_dir.path().join("session.jsonl");
    let log = Arc::new(JsonlSink {
        path: log_path.clone(),
    });
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let mut registry = BackendRegistry::new();
    registry
        .register(Arc::new(MockJudgeBackend::refusing(
            BackendId::Jev,
            JevError::input_too_large(4, 8),
        )))
        .expect("register");
    registry
        .register(Arc::clone(&haiku) as Arc<dyn JudgeBackend>)
        .expect("register");
    let error = run(prepare(
        check_file(),
        RunConfig::new(RepoRoot::new(repo.path()), log)
            .sessions_dir(sessions.path())
            .backends(set(&[BackendId::AnthropicHaiku, BackendId::Jev]))
            .registry(Arc::new(registry)),
    )
    .expect("prepare"))
    .expect_err("refused");
    assert_eq!(error.code(), "INPUT_TOO_LARGE");
    assert!(
        haiku.requests().is_empty(),
        "validation of every backend runs before the first send"
    );
    assert!(!log_path.exists());
    assert_eq!(std::fs::read_dir(sessions.path()).expect("dir").count(), 0);
}

#[test]
fn ask_capable_live_run_without_sessions_dir_is_config_invalid_before_send() {
    let repo = bare_repo();
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let error = run(prepare(
        check_file(),
        RunConfig::new(RepoRoot::new(repo.path()), Arc::new(NoopSessionLog))
            .backends(set(&[BackendId::AnthropicHaiku]))
            .registry(registry_of(&[&haiku])),
    )
    .expect("prepare"))
    .expect_err("no sessions dir");
    assert_eq!(error.code(), "CONFIG_INVALID");
    assert!(
        error.fix_hint().contains("sessions_dir"),
        "{}",
        error.fix_hint()
    );
    assert_eq!(haiku.validations(), 0);
    // A Jev-only live run saves no session, so it needs no directory.
    let jev = run(prepare(
        check_file(),
        RunConfig::new(RepoRoot::new(repo.path()), Arc::new(NoopSessionLog))
            .registry(registry_of(&[&judge(BackendId::Jev, "yes")])),
    )
    .expect("prepare"))
    .expect("jev run");
    assert!(!jev.session_saved());
    assert!(jev.session_id().is_none());
}

#[test]
fn unregistered_backend_is_a_registry_hint() {
    let repo = bare_repo();
    let error = run(prepare(
        check_file(),
        RunConfig::new(RepoRoot::new(repo.path()), Arc::new(NoopSessionLog))
            .backends(set(&[BackendId::Jev]))
            .registry(Arc::new(BackendRegistry::new())),
    )
    .expect("prepare"))
    .expect_err("not registered");
    assert_eq!(error.code(), "CONFIG_INVALID");
    assert!(error.fix_hint().contains("jev"), "{}", error.fix_hint());
    assert!(!error.fix_hint().contains("check.toml"));
}

#[test]
fn command_input_copies_producer() {
    let repo = test_support::temp_dir("command-run");
    std::fs::write(repo.path().join("questions.json"), QUESTIONS).expect("questions");
    let subjects = std::fs::read_to_string(
        test_support::workspace_root().join("tests/fixtures/jev/subjects-schema/v1/minimal.json"),
    )
    .expect("fixture");
    std::fs::write(repo.path().join("subjects.json"), subjects).expect("subjects");
    let python = test_support::python();
    let check = format!(
        "[questions]\npath = \"questions.json\"\n\n[inputs.live]\nkind = \"command\"\ncommand = [\"{python}\", \"-c\", \"print(open('subjects.json').read())\"]\ntimeout_seconds = 30\n"
    );
    std::fs::write(repo.path().join("check.toml"), check).expect("check");
    let prepared = prepare(
        check_file(),
        RunConfig::new(RepoRoot::new(repo.path()), Arc::new(NoopSessionLog))
            .registry(registry_of(&[&judge(BackendId::Jev, "yes")])),
    )
    .expect("prepare");
    assert_eq!(prepared.producer().expect("producer").name, "sc-treesitter");
    let response = run(prepared).expect("run");
    let producer = response.equal_input().producer().expect("producer");
    assert_eq!(producer.name, "sc-treesitter");
    assert_eq!(producer.language, "python");
}

#[test]
fn caller_request_uses_file_override_not_inline_prompt() {
    let repo = test_support::temp_dir("caller-prompt");
    let override_path = repo.path().join("override.md");
    std::fs::write(&override_path, "OVERRIDE-PROMPT-TOKEN-a7\n").expect("override");
    let inline = br#"{"questions":[{"id":"q","prompt":"p","choices":["no","yes"]}],"system_prompt":"INLINE-PROMPT-TOKEN"}"#;
    let rejected = CallerRequest::from_json(inline).expect_err("inline");
    assert!(!rejected.fix_hint().contains("INLINE-PROMPT-TOKEN"));
    assert!(rejected.fix_hint().contains("--llm-system-prompt-file"));
    let repeated = br#"{"questions":[{"id":"q","prompt":"p","choices":["no","yes"]}],"backends":["jev","jev"]}"#;
    assert_eq!(
        CallerRequest::from_json(repeated)
            .expect_err("repeat")
            .code(),
        "INPUT_INVALID"
    );

    let bytes = br#"{"questions":[{"id":"q","prompt":"Judge this","choices":["no","yes"]}],"backends":["anthropic_haiku"]}"#;
    let request = CallerRequest::from_json(bytes).expect("request");
    let base = RunConfig::new(RepoRoot::new(repo.path()), Arc::new(NoopSessionLog));
    let packaged =
        prepare(CheckSource::CallerRequest(request.clone()), base.clone()).expect("packaged");
    assert_eq!(packaged.backends().as_slice(), [BackendId::AnthropicHaiku]);
    assert_eq!(system_prompt(packaged.request()), PACKAGED_SYSTEM_PROMPT);
    let jev_only = prepare(
        CheckSource::CallerRequest(request.clone()),
        base.clone()
            .backends(set(&[BackendId::Jev]))
            .system_prompt_override(&override_path),
    )
    .expect("config backends win");
    assert_eq!(jev_only.backends().as_slice(), [BackendId::Jev]);
    assert!(jev_only.request().llm().is_none());
    let llm = prepare(
        CheckSource::CallerRequest(request),
        base.system_prompt_override(&override_path),
    )
    .expect("override");
    assert_eq!(system_prompt(llm.request()), "OVERRIDE-PROMPT-TOKEN-a7\n");
    assert_eq!(packaged.prompt_sha256(), llm.prompt_sha256());
    let text = llm.request().payload().as_str();
    assert!(!text.contains("OVERRIDE-PROMPT-TOKEN-a7"));
    assert!(text.contains("Judge this"));
}

#[test]
fn validate_for_send_repeats_only_for_context_bearing_requests() {
    let repo = bare_repo();
    let sessions = test_support::temp_dir("validate-sessions");
    let jev = judge(BackendId::Jev, "yes");
    let haiku = judge(BackendId::AnthropicHaiku, "no");
    run(prepare(
        check_file(),
        config(repo.path(), sessions.path())
            .backends(set(&[BackendId::Jev, BackendId::AnthropicHaiku]))
            .registry(registry_of(&[&jev, &haiku])),
    )
    .expect("prepare"))
    .expect("run");
    assert_eq!(jev.validations(), 1, "Jev: pre-flight only");
    assert_eq!(
        haiku.validations(),
        2,
        "Haiku: pre-flight, then again with session context"
    );
}

/// REQ-SAG-0610: one log line per judge send, under the run's session id.
#[test]
fn every_judge_send_logs_once_under_the_saved_session_id() {
    let repo = bare_repo();
    let sessions = test_support::temp_dir("log-sessions");
    let log_dir = test_support::temp_dir("log-jsonl");
    let log_path = log_dir.path().join("session.jsonl");
    let log = Arc::new(JsonlSink {
        path: log_path.clone(),
    });
    let response = run(prepare(
        check_file(),
        RunConfig::new(RepoRoot::new(repo.path()), log)
            .sessions_dir(sessions.path())
            .backends(set(&[BackendId::Jev, BackendId::OpenAiLuna]))
            .registry(registry_of(&[
                &judge(BackendId::Jev, "yes"),
                &judge(BackendId::OpenAiLuna, "no"),
            ])),
    )
    .expect("prepare"))
    .expect("run");
    let text = std::fs::read_to_string(&log_path).expect("jsonl");
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("line"))
        .collect();
    assert_eq!(lines.len(), 2);
    let session_id = response.session_id().expect("saved").to_string();
    for (line, backend) in lines.iter().zip(["jev", "openai_luna"]) {
        assert_eq!(line["session_id"], session_id);
        assert_eq!(line["backend"], backend);
        assert_eq!(line["prompt_sha256"], response.prompt_sha256().as_str());
        assert_eq!(line["operation"], "judge");
    }
}

#[test]
fn discover_repo_root_uses_the_git_toplevel() {
    let (dir, _sha) = test_support::git_fixture();
    let sub = dir.path().join("notes");
    let root = discover_repo_root(&sub).expect("root");
    assert_eq!(
        std::fs::canonicalize(root.as_path()).expect("canonical root"),
        std::fs::canonicalize(dir.path()).expect("canonical dir")
    );
}
