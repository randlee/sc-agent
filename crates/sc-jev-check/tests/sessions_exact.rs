//! Session and orchestration tests selected by `--exact`.
//!
//! The sprint a-6 acceptance commands run
//! `cargo test -p sc-jev-check --test sessions_exact -- <name> --exact`, so each function stays at
//! the root of this integration test and owns its name. The run and session
//! unit tests do not repeat these cases.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sc_agent_types::observability::v2::{
    LogEvent, LogRejected, NoopSessionLog, SessionLog, SharedSessionLog,
};
use sc_agent_types::test_util::MockJudgeBackend;
use sc_agent_types::{
    Attestation, BackendId, BackendSet, CanonicalJudgePayload, JudgeBackend, JudgeOutcome,
    JudgeRequest, MultiBackendMode, PayloadBundle, RepoRoot, Rev, SessionId,
};
use sc_jev_check::config::{PACKAGED_SYSTEM_PROMPT, load_check_at};
use sc_jev_check::registry::BackendRegistry;
use sc_jev_check::run::{CheckSource, PreparedRun, RunConfig, RunResponse, prepare, run};
use sc_jev_check::session::{FileSessionStore, SESSION_CONTEXT_HEADER, StoredSession};
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

/// The workspace judge double (`sc-agent-types/test-util`, enabled by this
/// crate's `sc-agent-types` dev-dependency). It records requests and answers
/// against the request it receives.
fn judge(id: BackendId, choice: &str) -> Arc<MockJudgeBackend> {
    let answers = serde_json::from_value(serde_json::json!({"relevant": {"choice": choice}}))
        .expect("answers");
    Arc::new(MockJudgeBackend::new(id, answers))
}

fn registry(judges: &[&Arc<MockJudgeBackend>]) -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    for judge in judges {
        registry
            .register(Arc::clone(*judge) as Arc<dyn JudgeBackend>)
            .expect("register");
    }
    registry
}

fn set(ids: &[BackendId]) -> BackendSet {
    BackendSet::try_from(ids.to_vec()).expect("backend set")
}

/// A live config against `root`, saving into `sessions`, logging to `log`.
fn live(root: RepoRoot, sessions: &Path, log: SharedSessionLog) -> RunConfig {
    RunConfig::new(root, log).sessions_dir(sessions)
}

fn run_spec(
    spec: sc_jev_check::config::CheckSpec,
    config: RunConfig,
) -> (PreparedRun, RunResponse) {
    let prepared = prepare(CheckSource::Spec(spec.clone()), config.clone()).expect("prepare");
    let response = run(prepare(CheckSource::Spec(spec), config).expect("prepare")).expect("run");
    (prepared, response)
}

struct JsonlSink {
    path: PathBuf,
}

impl SessionLog for JsonlSink {
    fn emit(&self, event: LogEvent) -> Result<(), LogRejected> {
        let line = serde_json::json!({
            "session_id": event.session_id().to_string(),
            "backend": event.backend().to_string(),
            "prompt_sha256": event.prompt_sha256().as_str(),
            "operation": event.operation().as_str(),
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
    let payload: CanonicalJudgePayload = serde_json::from_value(serde_json::json!({
        "questions": [{"id": "relevant", "prompt": "Does it apply?", "choices": ["no", "yes"]}],
        "state": []
    }))
    .expect("payload");
    PayloadBundle::encode(&payload).expect("bundle")
}

#[test]
fn test_session_round_trip() {
    let dir = temp_dir();
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let store = FileSessionStore::new(dir.path()).expect("store");
    let payload = bundle();
    let request = JudgeRequest::new(Arc::new(payload.clone()), None);
    let answers =
        serde_json::from_value(serde_json::json!({"relevant": {"choice": "no"}})).expect("answers");
    let id = SessionId::new();
    store
        .save(&StoredSession {
            session_id: id,
            backend: BackendId::AnthropicHaiku,
            created_at: store.now(),
            payload: payload.clone(),
            system_prompt: Some("base prompt".to_owned()),
            outcomes: vec![
                JudgeOutcome::new(BackendId::AnthropicHaiku, &request, answers).expect("outcome"),
            ],
            follow_ups: Vec::new(),
        })
        .expect("save");
    let loaded = store.load(&id).expect("load");
    assert_eq!(loaded.session_id, id);
    assert_eq!(loaded.payload.bytes(), payload.bytes());
    assert_eq!(loaded.payload.sha256(), payload.sha256());
    assert_eq!(loaded.system_prompt.as_deref(), Some("base prompt"));
    assert_eq!(loaded.outcomes.len(), 1);

    let reopened = FileSessionStore::new(store.dir()).expect("reopen");
    let log: SharedSessionLog = Arc::new(NoopSessionLog);
    let outcome = reopened
        .ask(&id, "why?", &registry(&[&haiku]), &log)
        .expect("ask");
    assert_eq!(outcome.backend(), BackendId::AnthropicHaiku);
    assert_eq!(outcome.answers()["relevant"].choice(), "yes");
    let requests = haiku.requests();
    let prompt = &requests[0].llm().expect("prompt").system_prompt;
    assert!(prompt.contains("base prompt"));
    assert!(prompt.contains("why?"));
    let again = store.load(&id).expect("reload");
    assert_eq!(again, loaded, "ask does not rewrite the session");
    let missing = SessionId::new();
    assert_eq!(
        store.load(&missing).expect_err("missing").code(),
        "SESSION_NOT_FOUND"
    );
}

#[test]
fn test_session_file_mode_0600() {
    let dir = temp_dir();
    let store = FileSessionStore::new(dir.path()).expect("store");
    let id = SessionId::new();
    store
        .save(&StoredSession {
            session_id: id,
            backend: BackendId::Jev,
            created_at: store.now(),
            payload: bundle(),
            system_prompt: None,
            outcomes: Vec::new(),
            follow_ups: Vec::new(),
        })
        .expect("save");
    let path = store.path(&id);
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(format!("{id}.json").as_str())
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

/// Test clock: a fixed instant the test moves forward.
static NOW_MS: AtomicU64 = AtomicU64::new(1_700_000_000_000);

fn test_clock() -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(NOW_MS.load(Ordering::SeqCst))
}

#[test]
fn test_session_expiry_uses_created_at() {
    let dir = temp_dir();
    let store = FileSessionStore::new(dir.path())
        .expect("store")
        .with_ttl(Duration::from_secs(60))
        .with_clock(test_clock);
    let id = SessionId::new();
    store
        .save(&StoredSession {
            session_id: id,
            backend: BackendId::Jev,
            created_at: store.now(),
            payload: bundle(),
            system_prompt: None,
            outcomes: Vec::new(),
            follow_ups: Vec::new(),
        })
        .expect("save");
    let path = store.path(&id);
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("json")).expect("parse");
    assert_eq!(document["created_at"], 1_700_000_000_000_u64);
    set_modified(&path, UNIX_EPOCH + Duration::from_secs(4_000_000_000));
    store.load(&id).expect("created_at is still fresh");
    NOW_MS.store(1_700_000_060_000, Ordering::SeqCst);
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
    let jev = judge(BackendId::Jev, "yes");
    let haiku = judge(BackendId::AnthropicHaiku, "no");
    let (prepared, result) = run_spec(
        spec,
        live(root, sessions.path(), Arc::new(NoopSessionLog))
            .backends(set(&[BackendId::Jev, BackendId::AnthropicHaiku]))
            .registry(Arc::new(registry(&[&jev, &haiku]))),
    );
    let equal = result.equal_input();
    assert_eq!(equal.prompt_sha256(), prepared.prompt_sha256());
    assert!(!equal.leak());
    // r-1 interim: no backend digest is trusted yet, so every entry is
    // `Unattested` and the derived `equal` is false (r-2b adds hashing).
    assert!(!equal.equal());
    assert_eq!(equal.mode(), MultiBackendMode::ChainContext);
    let ids: Vec<BackendId> = equal
        .backends()
        .iter()
        .map(|entry| entry.backend())
        .collect();
    assert_eq!(ids, [BackendId::Jev, BackendId::AnthropicHaiku]);
    assert!(
        equal
            .backends()
            .iter()
            .all(|entry| entry.attestation() == &Attestation::Unattested)
    );
    let producer = equal.producer().expect("producer");
    assert_eq!(producer.name, "sc-treesitter");
    assert_eq!(producer.mode, "matches");
    assert_eq!(
        jev.requests()[0].payload().bytes(),
        haiku.requests()[0].payload().bytes()
    );
}

#[test]
fn test_multi_backend_one_payload() {
    let repo = bare_repo();
    let root = RepoRoot::new(repo.path());
    let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("spec");
    let sessions = temp_dir();
    let jev = judge(BackendId::Jev, "yes");
    let haiku = judge(BackendId::AnthropicHaiku, "no");
    let config = live(root, sessions.path(), Arc::new(NoopSessionLog))
        .backends(set(&[BackendId::Jev, BackendId::AnthropicHaiku]))
        .registry(Arc::new(registry(&[&jev, &haiku])));
    let prepared = prepare(CheckSource::Spec(spec), config).expect("prepare");
    let payload = prepared.request().payload().clone();
    let result = run(prepared).expect("run");
    let jev_seen = jev.requests();
    let haiku_seen = haiku.requests();
    assert_eq!(jev_seen.len(), 1);
    assert_eq!(haiku_seen.len(), 1);
    assert_eq!(
        jev_seen[0].payload().bytes(),
        haiku_seen[0].payload().bytes()
    );
    assert_eq!(jev_seen[0].payload().bytes(), payload.bytes());
    assert!(jev_seen[0].llm().is_none());
    let prompt = &haiku_seen[0].llm().expect("prompt").system_prompt;
    assert!(prompt.starts_with(PACKAGED_SYSTEM_PROMPT));
    assert!(prompt.contains(SESSION_CONTEXT_HEADER));
    assert!(prompt.contains("\"choice\":\"yes\""));
    let payload_text = String::from_utf8_lossy(payload.bytes());
    assert!(!payload_text.contains(SESSION_CONTEXT_HEADER));
    assert!(!payload_text.contains(PACKAGED_SYSTEM_PROMPT.trim()));
    let equal = result.equal_input();
    assert!(
        !equal.equal(),
        "r-1 interim records Unattested for every backend"
    );
    assert!(!equal.leak());
    assert!(equal.producer().is_none());
    assert_eq!(equal.prompt_sha256(), payload.sha256());
    assert_eq!(result.results().len(), 2);
    let session_id = result.session_id().expect("session");
    let store = FileSessionStore::new(sessions.path()).expect("store");
    assert_eq!(store.load(&session_id).expect("stored").outcomes.len(), 2);
    let log: SharedSessionLog = Arc::new(NoopSessionLog);
    let asked = store
        .ask(
            &session_id,
            "why the first answer?",
            &registry(&[&haiku]),
            &log,
        )
        .expect("ask");
    assert_eq!(asked.backend(), BackendId::AnthropicHaiku);
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
    let result = run(prepare(
        CheckSource::Spec(spec),
        live(root, sessions.path(), Arc::clone(&log) as _)
            .backends(set(&[BackendId::Jev, BackendId::OpenAiLuna]))
            .system_prompt_override(repo.path().join("override.md"))
            .registry(Arc::new(registry(&[
                &judge(BackendId::Jev, "yes"),
                &judge(BackendId::OpenAiLuna, "no"),
            ]))),
    )
    .expect("prepare"))
    .expect("run");
    let text = fs::read_to_string(&log_path).expect("jsonl");
    assert!(!text.contains("sk-test-DO_NOT_LOG_9f3a"));
    assert!(!text.to_ascii_lowercase().contains("authorization"));
    assert!(!text.to_ascii_lowercase().contains("bearer"));
    let session_id = result.session_id().expect("session").to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    for (line, backend) in lines.iter().zip(["jev", "openai_luna"]) {
        let value: serde_json::Value = serde_json::from_str(line).expect("line");
        assert_eq!(value["session_id"], session_id);
        assert_eq!(value["backend"], backend);
        assert_eq!(value["prompt_sha256"], result.prompt_sha256().as_str());
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
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let config = live(root, sessions.path(), Arc::new(NoopSessionLog))
        .backends(set(&[BackendId::AnthropicHaiku]))
        .registry(Arc::new(registry(&[&haiku])));
    let prepared = prepare(CheckSource::Spec(spec), config).expect("prepare");
    assert_eq!(
        prepared
            .request()
            .llm()
            .map(|llm| llm.system_prompt.as_str()),
        Some(PACKAGED_SYSTEM_PROMPT)
    );
    let payload = prepared.request().payload().clone();
    let result = run(prepared).expect("run");
    let requests = haiku.requests();
    let prompt = &requests[0].llm().expect("llm").system_prompt;
    assert_eq!(prompt, PACKAGED_SYSTEM_PROMPT);
    assert!(!String::from_utf8_lossy(payload.bytes()).contains(PACKAGED_SYSTEM_PROMPT.trim()));
    assert_eq!(result.equal_input().prompt_sha256(), payload.sha256());
}
