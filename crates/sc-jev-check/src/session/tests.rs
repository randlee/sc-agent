//! Session store unit tests. Round-trip, `0600`, `created_at` expiry, and
//! the concurrency model live in `tests/sessions_exact.rs` and
//! `tests/session.rs` under their `--exact` names.

use std::sync::Arc;

use super::*;
use sc_agent_types::observability::v2::NoopSessionLog;

fn bundle() -> PayloadBundle {
    PayloadBundle::encode(&sc_agent_types::CanonicalJudgePayload::new(
        Vec::new(),
        Vec::new(),
    ))
    .expect("bundle")
}

fn store(ttl: Duration) -> (FileSessionStore, tempfile::TempDir) {
    let dir = crate::extract::test_support::temp_dir("session");
    let session = FileSessionStore::new(dir.path())
        .expect("store")
        .with_ttl(ttl);
    (session, dir)
}

fn session(backend: BackendId, payload: PayloadBundle, store: &FileSessionStore) -> StoredSession {
    StoredSession {
        session_id: SessionId::new(),
        backend,
        created_at: store.now(),
        payload,
        system_prompt: None,
        outcomes: Vec::new(),
        follow_ups: Vec::new(),
    }
}

#[test]
fn ask_refuses_empty_follow_up_and_jev_sessions_with_explicit_hints() {
    let (store, _dir) = store(Duration::from_secs(60));
    let saved = session(BackendId::Jev, bundle(), &store);
    store.save(&saved).expect("save");
    let registry = BackendRegistry::new();
    let log: SharedSessionLog = Arc::new(NoopSessionLog);
    let empty = store
        .ask(&saved.session_id, "  ", &registry, &log)
        .expect_err("empty");
    assert_eq!(empty.code(), "INPUT_INVALID");
    assert!(
        empty.fix_hint().contains("non-empty"),
        "{}",
        empty.fix_hint()
    );
    let jev = store
        .ask(&saved.session_id, "why?", &registry, &log)
        .expect_err("jev");
    assert_eq!(jev.code(), "INPUT_INVALID");
    assert!(
        jev.fix_hint().contains("Haiku or Luna"),
        "{}",
        jev.fix_hint()
    );
    assert!(!jev.fix_hint().contains("schema"), "{}", jev.fix_hint());
}

#[test]
fn session_parse_hint_names_the_file() {
    let (store, _dir) = store(Duration::from_secs(60));
    let saved = session(BackendId::Jev, bundle(), &store);
    store.save(&saved).expect("save");
    let path = store.path(&saved.session_id);
    fs::write(&path, b"not-json").expect("corrupt");
    let error = store.load(&saved.session_id).expect_err("parse");
    assert_eq!(error.code(), "INPUT_INVALID");
    let hint = error.fix_hint();
    assert!(
        hint.contains(&path.display().to_string()),
        "parse hint names the session file: {hint}"
    );
    assert!(
        !hint.contains("check.toml"),
        "parse hint is not a check.toml hint: {hint}"
    );
    assert!(
        !hint.to_ascii_lowercase().contains("schema"),
        "parse hint is not a schema hint: {hint}"
    );
}

#[test]
fn a_document_under_another_id_is_invalid() {
    let (store, _dir) = store(Duration::from_secs(60));
    let saved = session(BackendId::Jev, bundle(), &store);
    store.save(&saved).expect("save");
    let other = SessionId::new();
    fs::copy(store.path(&saved.session_id), store.path(&other)).expect("copy");
    let error = store.load(&other).expect_err("id mismatch");
    assert!(
        error.fix_hint().contains("does not match this session id"),
        "{}",
        error.fix_hint()
    );
}

#[test]
fn session_directory_failure_names_the_directory() {
    let parent = crate::extract::test_support::temp_dir("session-file-parent");
    let blocker = parent.path().join("not-a-directory");
    fs::write(&blocker, b"x").expect("blocker");
    let nested = blocker.join("sessions");
    let error = FileSessionStore::new(nested.as_path()).expect_err("dir");
    assert_eq!(error.code(), "CONFIG_INVALID");
    let hint = error.fix_hint();
    assert!(
        hint.contains(&nested.display().to_string()),
        "directory hint names the session directory: {hint}"
    );
    assert!(
        !hint.contains("check.toml"),
        "directory hint is not a check.toml hint: {hint}"
    );
}

#[test]
fn session_writes_leave_no_temporary_files() {
    let (store, dir) = store(Duration::from_secs(60));
    let mut saved = session(BackendId::Jev, bundle(), &store);
    store.save(&saved).expect("save");
    saved.outcomes.push(
        JudgeOutcome::new(
            BackendId::Jev,
            &JudgeRequest::new(Arc::new(bundle()), None),
            BTreeMap::new(),
        )
        .expect("outcome"),
    );
    store.save(&saved).expect("save again");
    let names: Vec<String> = fs::read_dir(dir.path())
        .expect("dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, [format!("{}.json", saved.session_id)]);
    assert_eq!(
        store.load(&saved.session_id).expect("load").outcomes.len(),
        1
    );
}

#[test]
fn temporary_names_are_unique_per_write() {
    let path = Path::new("/sessions/abc.json");
    let first = tmp_path(path);
    let second = tmp_path(path);
    assert_ne!(first, second);
    let name = first
        .file_name()
        .and_then(|name| name.to_str())
        .expect("name");
    assert!(
        name.starts_with(&format!("abc.json.tmp.{}.", std::process::id())),
        "{name}"
    );
}

#[cfg(unix)]
#[test]
fn created_session_directory_is_mode_0700() {
    use std::os::unix::fs::PermissionsExt;
    let parent = crate::extract::test_support::temp_dir("session-mode");
    let nested = parent.path().join("a").join("sessions");
    FileSessionStore::new(&nested).expect("store");
    let mode = fs::metadata(&nested).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

/// RBP-F015: clock and payload-invariant failures carry their own recovery,
/// not the generic `check.toml` / schema hints.
#[test]
fn clock_and_payload_faults_name_their_own_recovery() {
    let before_epoch = UNIX_EPOCH - Duration::from_secs(1);
    let clock = unix_ms(before_epoch).expect_err("before epoch");
    assert_eq!(clock.code(), "CONFIG_INVALID");
    let hint = clock.fix_hint();
    assert!(hint.contains("system clock"), "{hint}");
    assert!(!hint.contains("check.toml"), "{hint}");
}

/// RBP-F201: a stored outcome is checked against its stored payload on load.
#[test]
fn tampered_stored_outcome_is_session_document_invalid() {
    let (store, _dir) = store(Duration::from_secs(60));
    let payload = PayloadBundle::encode(&sc_agent_types::CanonicalJudgePayload::new(
        vec![sc_agent_types::Question::new(
            "relevant".parse().expect("id"),
            "Does it apply?",
            vec!["no".parse().expect("no"), "yes".parse().expect("yes")],
        )],
        Vec::new(),
    ))
    .expect("bundle");
    let request = JudgeRequest::new(Arc::new(payload.clone()), None);
    let answers = serde_json::from_value(serde_json::json!({"relevant": {"choice": "yes"}}))
        .expect("answers");
    let mut saved = session(BackendId::Jev, payload, &store);
    saved
        .outcomes
        .push(JudgeOutcome::new(BackendId::Jev, &request, answers).expect("outcome"));
    store.save(&saved).expect("save");
    store.load(&saved.session_id).expect("valid outcome loads");

    let path = store.path(&saved.session_id);
    let mut document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
    document["outcomes"][0]["answers"]["relevant"]["choice"] = serde_json::json!("maybe");
    fs::write(&path, serde_json::to_vec(&document).expect("bytes")).expect("tamper");
    let error = store.load(&saved.session_id).expect_err("tampered");
    assert!(
        matches!(error, JevError::SessionDocumentInvalid { .. }),
        "{error:?}"
    );
    assert!(error.fix_hint().contains("does not match its payload"));
}
