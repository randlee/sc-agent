//! Session and orchestration tests selected by `--exact`.
//!
//! The sprint a-6 acceptance commands run
//! `cargo test -p sc-jev-check -- <name> --exact`, so each function stays at
//! the root of this integration test and owns its name. The run and session
//! unit tests do not repeat these cases.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sc_agent_types::observability::v2::{EventError, LogEvent, NoopSessionLog, SessionLog};
use sc_agent_types::{
    BackendId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest, PayloadBundle, RepoRoot, Rev,
    SessionId,
};
use sc_jev_check::config::{PACKAGED_SYSTEM_PROMPT, load_check_at};
use sc_jev_check::registry::BackendRegistry;
use sc_jev_check::run::{RunOptions, run_check};
use sc_jev_check::session::{FileSessionStore, SESSION_CONTEXT_HEADER, SessionStore};
use tempfile::TempDir;

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

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("temp")
}

fn bare_repo() -> TempDir {
    let dir = temp_dir();
    fs::write(dir.path().join("questions.json"), QUESTIONS).expect("questions");
    fs::write(
        dir.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n",
    )
    .expect("check");
    dir
}

/// Requests seen by one or more [`Capture`] judges.
#[derive(Clone, Default)]
struct RequestLog(Arc<Mutex<Vec<JudgeRequest>>>);

impl RequestLog {
    fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> Vec<JudgeRequest> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Recording judge for this file only.
///
/// The crate's `ScriptedJudge` is behind its own `test-util` feature, and an
/// integration test cannot enable its own crate's feature without a self
/// dev-dependency (refused by the boundary lint and by crates.io publish).
/// This mirrors the `ScriptedJudge` calls the tests use.
#[derive(Clone)]
struct Capture {
    id: BackendId,
    choice: String,
    requests: RequestLog,
}

impl Capture {
    fn with_request_log(mut self, log: &RequestLog) -> Self {
        self.requests = log.clone();
        self
    }

    fn requests(&self) -> Vec<JudgeRequest> {
        self.requests.snapshot()
    }
}

impl JudgeBackend for Capture {
    fn id(&self) -> BackendId {
        self.id
    }

    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        if self.id.is_llm() && request.llm.is_none() {
            return Err(JevError::InputInvalid {
                detail: "LLM system prompt is required".into(),
            });
        }
        Ok(())
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.validate_for_send(request)?;
        self.requests
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Ok(JudgeOutcome {
            answers: serde_json::json!({"relevant": {"choice": self.choice}}),
            backend: self.id,
            probabilities: None,
            provider_meta: None,
        })
    }
}

fn judge(id: BackendId, choice: &str) -> Capture {
    Capture {
        id,
        choice: choice.to_owned(),
        requests: RequestLog::new(),
    }
}

fn registry(judges: &[&Capture]) -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    for judge in judges {
        registry
            .register(Arc::new((*judge).clone()))
            .expect("register");
    }
    registry
}

struct JsonlSink {
    path: PathBuf,
}

impl SessionLog for JsonlSink {
    fn try_log(&self, event: LogEvent) -> Result<(), EventError> {
        let line = serde_json::json!({
            "session_id": event.session_id.to_string(),
            "backend": event.backend.to_string(),
            "prompt_sha256": event.prompt_sha256.as_str(),
            "operation": event.operation,
        });
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .expect("jsonl");
        writeln!(file, "{line}").expect("write");
        Ok(())
    }
}

fn bundle() -> PayloadBundle {
    PayloadBundle::from_canonical_bytes(br#"{"questions":[],"state":[]}"#.to_vec())
}

#[test]
fn test_session_round_trip() {
    let dir = temp_dir();
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let store = FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog)).expect("store");
    store.install_registry(registry(&[&haiku]));
    let payload = bundle();
    let id = store
        .open(
            BackendId::AnthropicHaiku,
            &payload,
            Some("base prompt".to_owned()),
        )
        .expect("open");
    let loaded = store.load(&id).expect("load");
    assert_eq!(loaded.session_id, id);
    assert_eq!(loaded.payload.bytes(), payload.bytes());
    assert_eq!(loaded.payload.sha256(), payload.sha256());
    assert_eq!(loaded.system_prompt.as_deref(), Some("base prompt"));

    let reopened = FileSessionStore::new(store.dir(), Arc::new(NoopSessionLog)).expect("reopen");
    reopened.install_registry(registry(&[&haiku]));
    let outcome = reopened.ask(&id, "why?").expect("ask");
    assert_eq!(outcome.backend, BackendId::AnthropicHaiku);
    assert_eq!(outcome.answers["relevant"]["choice"], "yes");
    let requests = haiku.requests();
    let prompt = &requests[0].llm.as_ref().expect("prompt").system_prompt;
    assert!(prompt.contains("base prompt"));
    assert!(prompt.contains("why?"));
    let again = store.load(&id).expect("reload");
    assert_eq!(again.follow_ups, ["why?"]);
    assert_eq!(again.outcomes.len(), 1);
    let missing = SessionId::new();
    assert_eq!(
        store.load(&missing).expect_err("missing").code(),
        "SESSION_NOT_FOUND"
    );
}

#[test]
fn test_session_file_mode_0600() {
    let dir = temp_dir();
    let store = FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog)).expect("store");
    let id = store.create(BackendId::Jev, &bundle()).expect("create");
    let path = store.path(&id);
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(id.to_string().as_str())
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn test_session_expiry_uses_created_at() {
    let now = Arc::new(AtomicU64::new(1_700_000_000_000));
    let clock_now = Arc::clone(&now);
    let dir = temp_dir();
    let store = FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog))
        .expect("store")
        .with_ttl(Duration::from_secs(60))
        .with_clock(move || UNIX_EPOCH + Duration::from_millis(clock_now.load(Ordering::Relaxed)));
    let id = store.create(BackendId::Jev, &bundle()).expect("create");
    let path = store.path(&id);
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("json")).expect("parse");
    assert_eq!(document["created_at"], 1_700_000_000_000_u64);
    set_modified(&path, UNIX_EPOCH + Duration::from_secs(4_000_000_000));
    store.load(&id).expect("created_at is still fresh");
    now.store(1_700_000_060_000, Ordering::Relaxed);
    set_modified(&path, UNIX_EPOCH + Duration::from_millis(1_700_000_000_000));
    let error = store.load(&id).expect_err("expired from created_at");
    assert_eq!(error.code(), "SESSION_EXPIRED");
    assert!(error.fix_hint().contains(&id.to_string()));
}

fn set_modified(path: &Path, time: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .expect("open")
        .set_modified(time)
        .expect("mtime");
}

#[test]
fn test_equal_input_record() {
    let repo = temp_dir();
    fs::write(repo.path().join("questions.json"), QUESTIONS).expect("questions");
    let subjects = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/jev/subjects-schema/v1/minimal.json"),
    )
    .expect("subjects fixture");
    fs::write(repo.path().join("subjects.json"), subjects).expect("subjects");
    fs::write(
        repo.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n\n[inputs.branches]\nkind = \"subjects\"\npath = \"subjects.json\"\n",
    )
    .expect("check");
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let sessions = temp_dir();
    let store = FileSessionStore::new(sessions.path(), Arc::new(NoopSessionLog)).expect("store");
    let seen = RequestLog::new();
    let result = run_check(
        &spec,
        &[BackendId::Jev, BackendId::AnthropicHaiku],
        RunOptions::new(root, store, Arc::new(NoopSessionLog)).registry(registry(&[
            &judge(BackendId::Jev, "yes").with_request_log(&seen),
            &judge(BackendId::AnthropicHaiku, "no").with_request_log(&seen),
        ])),
    )
    .expect("run");
    let equal = result.equal_input.expect("record");
    assert_eq!(&equal.prompt_sha256, result.bundle.sha256());
    assert!(!equal.leak);
    assert!(equal.equal);
    assert_eq!(equal.commands, ["jev", "anthropic_haiku"]);
    let producer = equal.producer.expect("producer");
    assert_eq!(producer.name, "sc-treesitter");
    assert_eq!(producer.mode, "matches");
    let requests = seen.snapshot();
    assert_eq!(requests[0].payload.bytes(), requests[1].payload.bytes());
}

#[test]
fn test_multi_backend_one_payload() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let sessions = temp_dir();
    let store = FileSessionStore::new(sessions.path(), Arc::new(NoopSessionLog)).expect("store");
    let jev = judge(BackendId::Jev, "yes");
    let haiku = judge(BackendId::AnthropicHaiku, "no");
    let result = run_check(
        &spec,
        &[BackendId::Jev, BackendId::AnthropicHaiku],
        RunOptions::new(root, store.clone(), Arc::new(NoopSessionLog))
            .registry(registry(&[&jev, &haiku])),
    )
    .expect("run");
    let jev_seen = jev.requests();
    let haiku_seen = haiku.requests();
    assert_eq!(jev_seen.len(), 1);
    assert_eq!(haiku_seen.len(), 1);
    assert_eq!(jev_seen[0].payload.bytes(), haiku_seen[0].payload.bytes());
    assert_eq!(jev_seen[0].payload.bytes(), result.bundle.bytes());
    assert!(jev_seen[0].llm.is_none());
    let prompt = &haiku_seen[0].llm.as_ref().expect("prompt").system_prompt;
    assert!(prompt.starts_with(PACKAGED_SYSTEM_PROMPT));
    assert!(prompt.contains(SESSION_CONTEXT_HEADER));
    assert!(prompt.contains("\"choice\":\"yes\""));
    let payload_text = String::from_utf8_lossy(result.bundle.bytes());
    assert!(!payload_text.contains(SESSION_CONTEXT_HEADER));
    assert!(!payload_text.contains(PACKAGED_SYSTEM_PROMPT.trim()));
    let equal = result.equal_input.expect("equal");
    assert!(equal.equal);
    assert!(!equal.leak);
    assert!(equal.producer.is_none());
    assert_eq!(&equal.prompt_sha256, result.bundle.sha256());
    assert_eq!(result.outcomes.len(), 2);
    let session_id = result.session_id.expect("session");
    assert_eq!(store.load(&session_id).expect("stored").outcomes.len(), 2);
    let asked = store
        .ask(&session_id, "why the first answer?")
        .expect("ask");
    assert_eq!(asked.backend, BackendId::AnthropicHaiku);
}

#[test]
fn test_log_event_has_no_secrets() {
    let repo = bare_repo();
    fs::write(
        repo.path().join("override.md"),
        "instructions sk-test-DO_NOT_LOG_9f3a",
    )
    .expect("override");
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let log_dir = temp_dir();
    let log_path = log_dir.path().join("session.jsonl");
    let log = Arc::new(JsonlSink {
        path: log_path.clone(),
    });
    let sessions = temp_dir();
    let store = FileSessionStore::new(sessions.path(), Arc::clone(&log) as _).expect("store");
    let result = run_check(
        &spec,
        &[BackendId::Jev, BackendId::OpenAiLuna],
        RunOptions::new(root, store, Arc::clone(&log) as _)
            .system_prompt_override(repo.path().join("override.md"))
            .registry(registry(&[
                &judge(BackendId::Jev, "yes"),
                &judge(BackendId::OpenAiLuna, "no"),
            ])),
    )
    .expect("run");
    let text = fs::read_to_string(&log_path).expect("jsonl");
    assert!(!text.contains("sk-test-DO_NOT_LOG_9f3a"));
    assert!(!text.to_ascii_lowercase().contains("authorization"));
    assert!(!text.to_ascii_lowercase().contains("bearer"));
    let session_id = result.session_id.expect("session").to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    for (line, backend) in lines.iter().zip(["jev", "openai_luna"]) {
        let value: serde_json::Value = serde_json::from_str(line).expect("line");
        assert_eq!(value["session_id"], session_id);
        assert_eq!(value["backend"], backend);
        assert_eq!(value["prompt_sha256"], result.bundle.sha256().as_str());
        assert_eq!(value["operation"], "judge");
    }
}

#[test]
fn test_packaged_default_prompt_available() {
    assert!(PACKAGED_SYSTEM_PROMPT.contains("canonical judge payload"));
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let sessions = temp_dir();
    let store = FileSessionStore::new(sessions.path(), Arc::new(NoopSessionLog)).expect("store");
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let result = run_check(
        &spec,
        &[BackendId::AnthropicHaiku],
        RunOptions::new(root, store, Arc::new(NoopSessionLog)).registry(registry(&[&haiku])),
    )
    .expect("run");
    let requests = haiku.requests();
    let prompt = &requests[0].llm.as_ref().expect("llm").system_prompt;
    assert_eq!(prompt, PACKAGED_SYSTEM_PROMPT);
    assert_eq!(
        result.system_prompt.as_deref(),
        Some(PACKAGED_SYSTEM_PROMPT)
    );
    assert!(
        !String::from_utf8_lossy(result.bundle.bytes()).contains(PACKAGED_SYSTEM_PROMPT.trim())
    );
    assert_eq!(
        &result.equal_input.expect("equal").prompt_sha256,
        result.bundle.sha256()
    );
}
