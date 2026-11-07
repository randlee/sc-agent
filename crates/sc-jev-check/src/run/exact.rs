//! Sprint a-6 tests selected by `--exact`.
//!
//! The harness root is this file, so each function name is the full test name.

#![forbid(unsafe_code)]

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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

struct TempDir(PathBuf);

impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(label: &str) -> TempDir {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "sc-jev-check-exact-{label}-{}-{n}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temp");
    TempDir(path)
}

fn bare_repo() -> TempDir {
    let dir = temp_dir("repo");
    fs::write(dir.path().join("questions.json"), QUESTIONS).expect("questions");
    fs::write(
        dir.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n",
    )
    .expect("check");
    dir
}

struct Capture {
    id: BackendId,
    requests: Arc<Mutex<Vec<JudgeRequest>>>,
    choice: &'static str,
}

impl JudgeBackend for Capture {
    fn id(&self) -> BackendId {
        self.id
    }

    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        let needs_prompt = matches!(self.id, BackendId::AnthropicHaiku | BackendId::OpenAiLuna);
        if needs_prompt && request.llm.is_none() {
            return Err(JevError::InputInvalid {
                detail: "LLM system prompt is required".into(),
            });
        }
        Ok(())
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.validate_for_send(request)?;
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.clone());
        Ok(JudgeOutcome {
            answers: serde_json::json!({"relevant": {"choice": self.choice}}),
            backend: self.id,
            probabilities: None,
            provider_meta: None,
        })
    }
}

fn registry(captures: Vec<Capture>) -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    for capture in captures {
        registry.register(Arc::new(capture)).expect("register");
    }
    registry
}

fn capture(
    id: BackendId,
    requests: &Arc<Mutex<Vec<JudgeRequest>>>,
    choice: &'static str,
) -> Capture {
    Capture {
        id,
        requests: Arc::clone(requests),
        choice,
    }
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
    let dir = temp_dir("round");
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&prompts);
    struct Haiku {
        prompts: Arc<Mutex<Vec<String>>>,
    }
    impl JudgeBackend for Haiku {
        fn id(&self) -> BackendId {
            BackendId::AnthropicHaiku
        }
        fn validate_for_send(&self, _request: &JudgeRequest) -> Result<(), JevError> {
            Ok(())
        }
        fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
            self.prompts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(
                    request
                        .llm
                        .as_ref()
                        .map(|llm| llm.system_prompt.clone())
                        .unwrap_or_default(),
                );
            Ok(JudgeOutcome {
                answers: serde_json::json!({"relevant": {"choice": "yes"}}),
                backend: BackendId::AnthropicHaiku,
                probabilities: None,
                provider_meta: None,
            })
        }
    }
    let store = FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog)).expect("store");
    let mut backends = BackendRegistry::new();
    backends
        .register(Arc::new(Haiku { prompts: seen }))
        .expect("register");
    store.install_registry(backends);
    let payload = bundle();
    let id = store
        .open(
            BackendId::AnthropicHaiku,
            &payload,
            Some("base prompt".to_owned()),
        )
        .expect("open");
    let loaded = store.load(&id).expect("load");
    assert_eq!(loaded.payload.bytes(), payload.bytes());
    assert_eq!(loaded.payload.sha256(), payload.sha256());
    let outcome = store.ask(&id, "why?").expect("ask");
    assert_eq!(outcome.answers["relevant"]["choice"], "yes");
    let saved = prompts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(saved[0].contains("base prompt"));
    assert!(saved[0].contains("why?"));
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
    let dir = temp_dir("mode");
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
    let dir = temp_dir("expiry");
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
    let repo = temp_dir("equal");
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
    let sessions = temp_dir("equal-sessions");
    let store = FileSessionStore::new(sessions.path(), Arc::new(NoopSessionLog)).expect("store");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let result = run_check(
        &spec,
        &[BackendId::Jev, BackendId::AnthropicHaiku],
        RunOptions::new(root, store, Arc::new(NoopSessionLog)).registry(registry(vec![
            capture(BackendId::Jev, &seen, "yes"),
            capture(BackendId::AnthropicHaiku, &seen, "no"),
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
    let requests = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(requests[0].payload.bytes(), requests[1].payload.bytes());
}

#[test]
fn test_multi_backend_one_payload() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let sessions = temp_dir("multi-sessions");
    let store = FileSessionStore::new(sessions.path(), Arc::new(NoopSessionLog)).expect("store");
    let jev_seen = Arc::new(Mutex::new(Vec::new()));
    let haiku_seen = Arc::new(Mutex::new(Vec::new()));
    let result = run_check(
        &spec,
        &[BackendId::Jev, BackendId::AnthropicHaiku],
        RunOptions::new(root, store, Arc::new(NoopSessionLog)).registry(registry(vec![
            capture(BackendId::Jev, &jev_seen, "yes"),
            capture(BackendId::AnthropicHaiku, &haiku_seen, "no"),
        ])),
    )
    .expect("run");
    let jev = jev_seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let haiku = haiku_seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(jev[0].payload.bytes(), haiku[0].payload.bytes());
    assert_eq!(jev[0].payload.bytes(), result.bundle.bytes());
    assert!(jev[0].llm.is_none());
    let prompt = &haiku[0].llm.as_ref().expect("prompt").system_prompt;
    assert!(prompt.starts_with(PACKAGED_SYSTEM_PROMPT));
    assert!(prompt.contains(SESSION_CONTEXT_HEADER));
    assert!(prompt.contains("\"choice\":\"yes\""));
    let equal = result.equal_input.expect("equal");
    assert!(equal.equal);
    assert_eq!(&equal.prompt_sha256, result.bundle.sha256());
    assert_eq!(result.outcomes.len(), 2);
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
    let log_dir = temp_dir("jsonl");
    let log_path = log_dir.path().join("session.jsonl");
    let log = Arc::new(JsonlSink {
        path: log_path.clone(),
    });
    let sessions = temp_dir("log-sessions");
    let store = FileSessionStore::new(sessions.path(), Arc::clone(&log) as _).expect("store");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let result = run_check(
        &spec,
        &[BackendId::Jev, BackendId::OpenAiLuna],
        RunOptions::new(root, store, Arc::clone(&log) as _)
            .system_prompt_override(repo.path().join("override.md"))
            .registry(registry(vec![
                capture(BackendId::Jev, &seen, "yes"),
                capture(BackendId::OpenAiLuna, &seen, "no"),
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
    assert!(PACKAGED_SYSTEM_PROMPT.contains("PACKAGED-PROMPT-TOKEN-a3"));
    assert!(PACKAGED_SYSTEM_PROMPT.contains("canonical judge payload"));
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let sessions = temp_dir("prompt-sessions");
    let store = FileSessionStore::new(sessions.path(), Arc::new(NoopSessionLog)).expect("store");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let result =
        run_check(
            &spec,
            &[BackendId::AnthropicHaiku],
            RunOptions::new(root, store, Arc::new(NoopSessionLog)).registry(registry(vec![
                capture(BackendId::AnthropicHaiku, &seen, "yes"),
            ])),
        )
        .expect("run");
    let seen = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let prompt = &seen[0].llm.as_ref().expect("llm").system_prompt;
    assert_eq!(prompt, PACKAGED_SYSTEM_PROMPT);
    assert_eq!(
        result.system_prompt.as_deref(),
        Some(PACKAGED_SYSTEM_PROMPT)
    );
    assert!(!String::from_utf8_lossy(result.bundle.bytes()).contains("PACKAGED-PROMPT-TOKEN-a3"));
    assert_eq!(
        &result.equal_input.expect("equal").prompt_sha256,
        result.bundle.sha256()
    );
}
