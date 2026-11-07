//! Session store: document versioning (r-1 D7.6, REQ-SAG-0600) and the
//! stateless store with its no-lock concurrency model (REQ-SAG-0607, D4).
//!
//! Version 2 is the only accepted session document. A Phase A (version 1)
//! file and a file written by a newer sc-agent both load as
//! `SessionDocumentInvalid` (`INPUT_INVALID`) with their own recovery text.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sc_agent_types::observability::v2::{NoopSessionLog, SharedSessionLog};
use sc_agent_types::test_util::MockJudgeBackend;
use sc_agent_types::{
    BackendId, BackendSet, CanonicalJudgePayload, JevError, JudgeBackend, JudgeOutcome,
    JudgeRequest, PayloadBundle, RepoRoot, SessionId,
};
use sc_jev_check::registry::BackendRegistry;
use sc_jev_check::run::{CheckSource, RunConfig, prepare, run};
use sc_jev_check::session::{FileSessionStore, STALE_TMP_AGE, StoredSession};
use tempfile::TempDir;

const V1_SESSION_ID: &str = "0b7c9d1e-2f3a-4b5c-8d6e-7f8091a2b3c4";

fn fixture() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session-v1.json"),
    )
    .expect("session-v1.json")
}

/// A store whose clock sits just after the fixture's `created_at`, so the
/// only possible failure is the document version.
fn store(dir: &tempfile::TempDir) -> FileSessionStore {
    fn clock() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_001_000)
    }
    FileSessionStore::new(dir.path())
        .expect("store")
        .with_clock(clock)
}

fn load(text: &str) -> JevError {
    let dir = tempfile::tempdir().expect("temp");
    let store = store(&dir);
    let id: SessionId = serde_json::from_value(serde_json::json!(V1_SESSION_ID)).expect("id");
    std::fs::write(store.path(&id), text).expect("write session");
    store.load(&id).expect_err("refused")
}

#[test]
fn test_phase_a_session_is_invalid() {
    let error = load(&fixture());
    assert!(
        matches!(error, JevError::SessionDocumentInvalid { .. }),
        "{error:?}"
    );
    assert_eq!(error.code(), "INPUT_INVALID");
    let hint = error.fix_hint();
    assert!(
        hint.contains("re-run the check; Phase A session files cannot be resumed"),
        "{hint}"
    );
    assert!(hint.contains(V1_SESSION_ID), "hint names the file: {hint}");

    let mut unversioned: serde_json::Value = serde_json::from_str(&fixture()).expect("json");
    unversioned
        .as_object_mut()
        .expect("object")
        .remove("version");
    let error = load(&unversioned.to_string());
    assert!(
        error
            .fix_hint()
            .contains("Phase A session files cannot be resumed"),
        "{error:?}"
    );
}

#[test]
fn test_newer_session_version_is_invalid() {
    let mut newer: serde_json::Value = serde_json::from_str(&fixture()).expect("json");
    newer["version"] = serde_json::json!(3);
    let error = load(&newer.to_string());
    assert!(
        matches!(error, JevError::SessionDocumentInvalid { .. }),
        "{error:?}"
    );
    assert_eq!(error.code(), "INPUT_INVALID");
    assert!(
        error
            .fix_hint()
            .contains("written by a newer sc-agent; upgrade or re-run the check"),
        "{}",
        error.fix_hint()
    );
}

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

fn temp(label: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sc-jev-check-session-{label}-"))
        .tempdir()
        .expect("temp")
}

fn repo() -> TempDir {
    let dir = temp("repo");
    fs::write(dir.path().join("questions.json"), QUESTIONS).expect("questions");
    fs::write(
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

fn registry(judges: &[&Arc<MockJudgeBackend>]) -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    for judge in judges {
        registry
            .register(Arc::clone(*judge) as Arc<dyn JudgeBackend>)
            .expect("register");
    }
    registry
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// One live run of `backends` against `repo`, saving into `sessions`.
fn live_run(
    repo: &Path,
    sessions: &Path,
    backends: &[BackendId],
    judges: &[&Arc<MockJudgeBackend>],
) -> sc_jev_check::run::RunResponse {
    let config = RunConfig::new(RepoRoot::new(repo), Arc::new(NoopSessionLog))
        .backends(BackendSet::try_from(backends.to_vec()).expect("set"))
        .sessions_dir(sessions)
        .registry(Arc::new(registry(judges)));
    run(prepare(CheckSource::CheckFile("check.toml".into()), config).expect("prepare"))
        .expect("run")
}

fn payload() -> PayloadBundle {
    let payload: CanonicalJudgePayload = serde_json::from_value(serde_json::json!({
        "questions": [{"id": "relevant", "prompt": "Does it apply?", "choices": ["no", "yes"]}],
        "state": []
    }))
    .expect("payload");
    PayloadBundle::encode(&payload).expect("bundle")
}

fn stored(store: &FileSessionStore, id: SessionId, choice: &str) -> StoredSession {
    let bundle = payload();
    let request = JudgeRequest::new(Arc::new(bundle.clone()), None);
    let answers = serde_json::from_value(serde_json::json!({"relevant": {"choice": choice}}))
        .expect("answers");
    StoredSession {
        session_id: id,
        backend: BackendId::AnthropicHaiku,
        created_at: store.now(),
        payload: bundle,
        system_prompt: Some(format!("prompt-{choice}")),
        outcomes: vec![
            JudgeOutcome::new(BackendId::AnthropicHaiku, &request, answers).expect("outcome"),
        ],
        follow_ups: Vec::new(),
    }
}

#[test]
fn test_session_written_once_only_with_llm() {
    let repo = repo();
    let sessions = temp("once");
    let jev_only = live_run(
        repo.path(),
        sessions.path(),
        &[BackendId::Jev],
        &[&judge(BackendId::Jev, "yes")],
    );
    assert!(!jev_only.session_saved());
    assert!(jev_only.session_id().is_none());
    assert!(
        names(sessions.path()).is_empty(),
        "a run with no ask-capable backend writes no session"
    );

    let jev = judge(BackendId::Jev, "yes");
    let haiku = judge(BackendId::AnthropicHaiku, "no");
    let both = live_run(
        repo.path(),
        sessions.path(),
        &[BackendId::Jev, BackendId::AnthropicHaiku],
        &[&jev, &haiku],
    );
    assert!(both.session_saved());
    let id = both.session_id().expect("saved");
    assert_eq!(
        names(sessions.path()),
        [format!("{id}.json")],
        "one file, no temporary leftovers"
    );
    let store = FileSessionStore::new(sessions.path()).expect("store");
    let loaded = store.load(&id).expect("load");
    assert_eq!(loaded.backend, BackendId::AnthropicHaiku);
    assert_eq!(
        loaded
            .outcomes
            .iter()
            .map(JudgeOutcome::backend)
            .collect::<Vec<_>>(),
        [BackendId::Jev, BackendId::AnthropicHaiku],
        "the one write happens after the last backend"
    );
    assert_eq!(loaded.payload.sha256(), both.prompt_sha256());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(store.path(&id))
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn test_ask_takes_registry() {
    let repo = repo();
    let sessions = temp("ask");
    let run_judge = judge(BackendId::AnthropicHaiku, "yes");
    let response = live_run(
        repo.path(),
        sessions.path(),
        &[BackendId::AnthropicHaiku],
        &[&run_judge],
    );
    let id = response.session_id().expect("saved");
    let store = FileSessionStore::new(sessions.path()).expect("fresh store, no state");
    let path = store.path(&id);
    let before = fs::read(&path).expect("session bytes");

    let ask_judge = judge(BackendId::AnthropicHaiku, "no");
    let log: SharedSessionLog = Arc::new(NoopSessionLog);
    let outcome = store
        .ask(&id, "why the choice?", &registry(&[&ask_judge]), &log)
        .expect("ask");
    assert_eq!(outcome.answers()["relevant"].choice(), "no");
    assert_eq!(
        run_judge.requests().len(),
        1,
        "the run's judge is not reused"
    );
    let requests = ask_judge.requests();
    assert_eq!(requests.len(), 1, "ask uses the registry it is given");
    let prompt = &requests[0].llm().expect("prompt").system_prompt;
    assert!(prompt.contains("why the choice?"), "{prompt}");
    assert!(prompt.contains("Session context:"), "{prompt}");
    assert_eq!(
        fs::read(&path).expect("session bytes"),
        before,
        "ask does not rewrite the file"
    );

    let missing = store
        .ask(&id, "again?", &BackendRegistry::new(), &log)
        .expect_err("backend not in the registry");
    assert_eq!(missing.code(), "CONFIG_INVALID");
}

#[test]
fn test_two_stores_same_dir_last_writer_wins() {
    let dir = temp("two");
    let first = FileSessionStore::new(dir.path()).expect("first");
    let second = FileSessionStore::new(dir.path()).expect("second");
    let id = SessionId::new();
    first.save(&stored(&first, id, "yes")).expect("first save");
    second
        .save(&stored(&second, id, "no"))
        .expect("second save");
    let loaded = first.load(&id).expect("load");
    assert_eq!(loaded.outcomes[0].answers()["relevant"].choice(), "no");
    assert_eq!(loaded.system_prompt.as_deref(), Some("prompt-no"));

    // Concurrent writers of one id: every load in between sees a whole,
    // verified document, and the final file is one writer's. POSIX rename
    // replaces atomically under concurrency; on Windows a concurrent replace
    // may fail with a sharing error instead (module docs), so this half is
    // Unix-only.
    #[cfg(unix)]
    concurrent_writers(&first, &second, id);
    assert_eq!(
        names(dir.path()),
        [format!("{id}.json")],
        "no temporary leftovers"
    );
}

#[cfg(unix)]
fn concurrent_writers(first: &FileSessionStore, second: &FileSessionStore, id: SessionId) {
    let writers: Vec<_> = (0..8)
        .map(|index| {
            let store = if index % 2 == 0 {
                first.clone()
            } else {
                second.clone()
            };
            let choice = if index % 2 == 0 { "yes" } else { "no" };
            std::thread::spawn(move || {
                for _ in 0..10 {
                    store.save(&stored(&store, id, choice)).expect("save");
                    store.load(&id).expect("a reader never sees a torn file");
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("writer");
    }
    let last = second.load(&id).expect("final");
    let choice = last.outcomes[0].answers()["relevant"]
        .choice()
        .as_str()
        .to_owned();
    assert!(choice == "yes" || choice == "no", "{choice}");
}

#[test]
fn test_save_error_removes_tmp() {
    let dir = temp("tmp");
    let store = FileSessionStore::new(dir.path()).expect("store");
    let id = SessionId::new();
    // A non-empty directory at `<id>.json` makes the final rename fail
    // after the temporary file was written.
    let blocker = store.path(&id);
    fs::create_dir(&blocker).expect("blocker");
    fs::write(blocker.join("keep"), b"x").expect("keep");
    let error = store
        .save(&stored(&store, id, "yes"))
        .expect_err("rename fails");
    assert_eq!(error.code(), "CONFIG_INVALID");
    assert!(
        error.fix_hint().contains(&blocker.display().to_string()),
        "{}",
        error.fix_hint()
    );
    assert_eq!(
        names(dir.path()),
        [format!("{id}.json")],
        "the temporary file is removed"
    );
}

#[test]
fn test_stale_tmp_swept() {
    let dir = temp("sweep");
    let id = SessionId::new();
    let stale = dir.path().join(format!("{id}.json.tmp.1.00000000deadbeef"));
    let fresh = dir.path().join(format!("{id}.json.tmp.2.00000000cafef00d"));
    let session = dir.path().join(format!("{id}.json"));
    for path in [&stale, &fresh, &session] {
        fs::write(path, b"{}").expect("write");
    }
    let old = SystemTime::now() - STALE_TMP_AGE - Duration::from_secs(60);
    for path in [&stale, &session] {
        fs::File::options()
            .write(true)
            .open(path)
            .expect("open")
            .set_modified(old)
            .expect("mtime");
    }
    FileSessionStore::new(dir.path()).expect("store sweeps on open");
    assert!(
        !stale.exists(),
        "a temporary file older than one hour is removed"
    );
    assert!(
        fresh.exists(),
        "a recent temporary file may belong to a live writer"
    );
    assert!(session.exists(), "session files are never swept");
}
