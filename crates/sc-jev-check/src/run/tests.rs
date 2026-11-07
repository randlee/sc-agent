//! Run facade unit tests. Multi-backend payload sharing, equal-input, log
//! lines, and the packaged prompt default live in `tests/sessions_exact.rs`
//! under their `--exact` names.

use super::*;
use crate::config::{PACKAGED_SYSTEM_PROMPT, load_check_at};
use crate::extract::test_support::{self, copy_fixture};
use crate::registry::ScriptedJudge;
use sc_agent_types::observability::v2::{EventError, LogEvent, NoopSessionLog, SessionLog};
use std::path::PathBuf;
use std::sync::Arc;

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
    fn try_log(&self, event: LogEvent) -> Result<(), EventError> {
        use std::io::Write;
        let line = serde_json::json!({
            "session_id": event.session_id.to_string(),
            "backend": event.backend.to_string(),
            "prompt_sha256": event.prompt_sha256.as_str(),
            "operation": event.operation,
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

fn sessions_for(log: SharedSessionLog) -> (FileSessionStore, tempfile::TempDir) {
    let dir = test_support::temp_dir("run-sessions");
    let store = FileSessionStore::new(dir.path(), Arc::clone(&log)).expect("sessions");
    (store, dir)
}

fn judge(id: BackendId, choice: &str) -> ScriptedJudge {
    ScriptedJudge::new(id, serde_json::json!({"relevant": {"choice": choice}}))
}

fn registry_of(judges: &[&ScriptedJudge]) -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    for judge in judges {
        registry
            .register(Arc::new((*judge).clone()))
            .expect("register");
    }
    registry
}

fn system_prompt(request: &JudgeRequest) -> &str {
    &request.llm.as_ref().expect("llm prompt").system_prompt
}

#[test]
fn run_dry_run_returns_digest_without_sending() {
    let repo = copy_fixture();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let jev = judge(BackendId::Jev, "yes");
    let result = run_check(
        &spec,
        &[BackendId::Jev],
        RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
            .dry_run(true)
            .registry(registry_of(&[&jev])),
    )
    .expect("dry-run");
    assert!(result.session_id.is_none());
    assert!(result.outcomes.is_empty());
    assert!(result.equal_input.is_none());
    assert_eq!(result.bundle.sha256().as_str().len(), 64);
    assert!(jev.requests().is_empty());
}

#[test]
fn run_packaged_prompt_is_injected_and_override_does_not_change_digest() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    std::fs::write(repo.path().join("override.md"), "OVERRIDE-PROMPT-TOKEN").expect("override");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let first_judge = judge(BackendId::AnthropicHaiku, "yes");
    let first = run_check(
        &spec,
        &[BackendId::AnthropicHaiku],
        RunOptions::new(root.clone(), sessions, Arc::new(NoopSessionLog))
            .registry(registry_of(&[&first_judge])),
    )
    .expect("packaged");
    assert_eq!(
        system_prompt(&first_judge.requests()[0]),
        PACKAGED_SYSTEM_PROMPT
    );
    let second_judge = judge(BackendId::AnthropicHaiku, "yes");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let second = run_check(
        &spec,
        &[BackendId::AnthropicHaiku],
        RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
            .system_prompt_override(repo.path().join("override.md"))
            .registry(registry_of(&[&second_judge])),
    )
    .expect("override");
    assert_eq!(
        system_prompt(&second_judge.requests()[0]),
        "OVERRIDE-PROMPT-TOKEN"
    );
    assert_eq!(first.bundle.sha256(), second.bundle.sha256());
}

#[test]
fn run_check_file_prompt_beats_packaged_asset() {
    let repo = copy_fixture();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let luna = judge(BackendId::OpenAiLuna, "yes");
    run_check(
        &spec,
        &[BackendId::OpenAiLuna],
        RunOptions::new(root, sessions, Arc::new(NoopSessionLog)).registry(registry_of(&[&luna])),
    )
    .expect("run");
    let requests = luna.requests();
    let prompt = system_prompt(&requests[0]);
    assert!(prompt.contains("PROMPT-TOKEN-a3-check"));
    assert!(!prompt.contains(PACKAGED_SYSTEM_PROMPT.trim()));
}

#[test]
fn run_missing_llm_prompt_does_not_send() {
    let repo = bare_repo();
    std::fs::write(
        repo.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n\n[llm_judge]\nsystem_prompt_file = \"missing.md\"\n",
    )
    .expect("check");
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let (sessions, session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let error = run_check(
        &spec,
        &[BackendId::AnthropicHaiku],
        RunOptions::new(root.clone(), sessions, Arc::new(NoopSessionLog))
            .registry(registry_of(&[&haiku])),
    )
    .expect_err("missing prompt");
    assert_eq!(error.code(), "PROMPT_FILE_MISSING");
    assert!(haiku.requests().is_empty());
    assert_eq!(
        std::fs::read_dir(session_dir.path()).expect("dir").count(),
        0
    );
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let jev = run_check(
        &spec,
        &[BackendId::Jev],
        RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
            .registry(registry_of(&[&judge(BackendId::Jev, "yes")])),
    )
    .expect("jev ignores missing prompt");
    assert!(jev.system_prompt.is_none());
    assert!(jev.outcomes[0].answers["relevant"]["choice"] == "yes");
}

#[test]
fn run_refusing_backend_does_not_log() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let log_dir = test_support::temp_dir("jsonl");
    let log_path = log_dir.path().join("session.jsonl");
    let log = Arc::new(JsonlSink {
        path: log_path.clone(),
    });
    let (sessions, _session_dir) = sessions_for(Arc::clone(&log) as SharedSessionLog);
    let mut registry = BackendRegistry::new();
    registry
        .register(Arc::new(
            sc_agent_types::test_util::MockJudgeBackend::refusing(
                BackendId::Jev,
                JevError::InputTooLarge {
                    limit_bytes: 4,
                    actual_bytes: 8,
                },
            ),
        ))
        .expect("register");
    let error = run_check(
        &spec,
        &[BackendId::Jev],
        RunOptions::new(root, sessions, log).registry(registry),
    )
    .expect_err("refused");
    assert_eq!(error.code(), "INPUT_TOO_LARGE");
    assert!(!log_path.exists());
}

#[test]
fn run_request_missing_llm_prompt_is_config_invalid() {
    let dir = test_support::temp_dir("run-request-prompt");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let error = run_request(
        &RunRequest {
            bundle: PayloadBundle::from_canonical_bytes(br#"{"questions":[],"state":[]}"#.to_vec()),
            system_prompt: None,
            producer: None,
        },
        &[BackendId::OpenAiLuna],
        RunOptions::new(
            RepoRoot::new(dir.path()),
            sessions,
            Arc::new(NoopSessionLog),
        ),
    )
    .expect_err("missing prompt");
    assert_eq!(error.code(), "CONFIG_INVALID");
    assert!(
        !matches!(error, JevError::PromptFileMissing { .. }),
        "a missing run-request prompt is not a prompt-file path"
    );
    let hint = error.fix_hint();
    assert!(hint.contains("system prompt"), "{hint}");
    assert!(!hint.contains("at system prompt"), "{hint}");
    assert!(!hint.contains("check.toml field"), "{hint}");
}

#[test]
fn run_without_backends_is_a_registry_hint() {
    let dir = test_support::temp_dir("no-backends");
    let error = run_request(
        &RunRequest {
            bundle: PayloadBundle::from_canonical_bytes(br#"{"questions":[],"state":[]}"#.to_vec()),
            system_prompt: None,
            producer: None,
        },
        &[],
        RunOptions::dry_run_only(RepoRoot::new(dir.path())),
    )
    .expect_err("no backends");
    assert_eq!(error.code(), "CONFIG_INVALID");
    assert!(error.fix_hint().contains("at least one backend"));
    assert!(!error.fix_hint().contains("check.toml"));
}

#[test]
fn run_command_input_copies_producer() {
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
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let result = run_check(
        &spec,
        &[BackendId::Jev],
        RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
            .registry(registry_of(&[&judge(BackendId::Jev, "yes")])),
    )
    .expect("run");
    let producer = result
        .equal_input
        .expect("equal")
        .producer
        .expect("producer");
    assert_eq!(producer.name, "sc-treesitter");
    assert_eq!(producer.language, "python");
}

#[test]
fn build_request_assembles_canonical_payload_and_producer() {
    let repo = test_support::temp_dir("build-request");
    std::fs::write(repo.path().join("questions.json"), QUESTIONS).expect("questions");
    let subjects = std::fs::read_to_string(
        test_support::workspace_root().join("tests/fixtures/jev/subjects-schema/v1/minimal.json"),
    )
    .expect("fixture");
    std::fs::write(repo.path().join("subjects.json"), subjects).expect("subjects");
    std::fs::write(
        repo.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n\n[inputs.branches]\nkind = \"subjects\"\npath = \"subjects.json\"\n",
    )
    .expect("check");
    let root = RepoRoot::new(repo.path());
    let built = build_request(&root, &Rev::worktree(), "check.toml").expect("build");
    let again = build_request(&root, &Rev::worktree(), "check.toml").expect("rebuild");
    assert_eq!(built.bundle.sha256(), again.bundle.sha256());
    assert_eq!(
        built.bundle.sha256(),
        &sc_agent_types::Sha256Digest::of(built.bundle.bytes())
    );
    let producer = built.producer.expect("producer");
    assert_eq!(producer.name, "sc-treesitter");
    assert_eq!(producer.mode, "matches");
    let missing = build_request(&root, &Rev::worktree(), "missing.toml").expect_err("missing");
    assert_eq!(missing.code(), "INPUT_MISSING");
}

#[test]
fn prepare_caller_request_uses_file_override_not_inline_prompt() {
    let repo = test_support::temp_dir("caller-prompt");
    let root = RepoRoot::new(repo.path());
    let override_path = repo.path().join("override.md");
    std::fs::write(&override_path, "OVERRIDE-PROMPT-TOKEN-a7\n").expect("override");
    let inline = br#"{"questions":[{"id":"q","prompt":"p","choices":["no","yes"]}],"system_prompt":"INLINE-PROMPT-TOKEN"}"#;
    let rejected = prepare_caller_request(inline, &[], &root, None).expect_err("inline");
    assert!(!rejected.fix_hint().contains("INLINE-PROMPT-TOKEN"));
    assert!(rejected.fix_hint().contains("--llm-system-prompt-file"));

    let bytes = br#"{"questions":[{"id":"q","prompt":"Judge this","choices":["no","yes"]}],"backends":["anthropic_haiku"]}"#;
    let packaged = prepare_caller_request(bytes, &[], &root, None).expect("packaged");
    assert_eq!(packaged.backends, vec![BackendId::AnthropicHaiku]);
    assert_eq!(
        packaged.system_prompt.as_deref(),
        Some(PACKAGED_SYSTEM_PROMPT)
    );
    let jev_only = prepare_caller_request(
        bytes,
        &[BackendId::Jev],
        &root,
        Some(override_path.as_path()),
    )
    .expect("flags win");
    assert_eq!(jev_only.backends, vec![BackendId::Jev]);
    assert!(jev_only.system_prompt.is_none());
    let llm = prepare_caller_request(
        bytes,
        &[BackendId::AnthropicHaiku],
        &root,
        Some(override_path.as_path()),
    )
    .expect("override");
    assert_eq!(
        llm.system_prompt.as_deref(),
        Some("OVERRIDE-PROMPT-TOKEN-a7\n")
    );
    assert_eq!(packaged.bundle.sha256(), llm.bundle.sha256());
    let text = std::str::from_utf8(llm.bundle.bytes()).expect("utf8");
    assert!(!text.contains("OVERRIDE-PROMPT-TOKEN-a7"));
    assert!(text.contains("Judge this"));
}

#[test]
fn dry_run_only_needs_no_store_and_writes_nothing() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let before = std::fs::read_dir(repo.path()).expect("dir").count();
    let result = run_check(
        &spec,
        &[BackendId::Jev],
        RunOptions::dry_run_only(root.clone()),
    )
    .expect("dry-run");
    assert!(result.session_id.is_none());
    assert_eq!(std::fs::read_dir(repo.path()).expect("dir").count(), before);
    let live = run_check(
        &spec,
        &[BackendId::Jev],
        RunOptions::dry_run_only(root).dry_run(false),
    )
    .expect_err("live run needs sessions");
    assert_eq!(live.code(), "CONFIG_INVALID");
    assert!(live.fix_hint().contains("session store"));
}

#[test]
fn validate_for_send_repeats_only_for_context_bearing_requests() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
    let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
    let jev = judge(BackendId::Jev, "yes");
    let haiku = judge(BackendId::AnthropicHaiku, "no");
    run_check(
        &spec,
        &[BackendId::Jev, BackendId::AnthropicHaiku],
        RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
            .registry(registry_of(&[&jev, &haiku])),
    )
    .expect("run");
    assert_eq!(jev.validations(), 1, "Jev: pre-flight only");
    assert_eq!(
        haiku.validations(),
        2,
        "Haiku: pre-flight, then again with session context"
    );
}
