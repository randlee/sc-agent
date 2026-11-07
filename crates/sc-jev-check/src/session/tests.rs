//! Session store unit tests. Round-trip, `0600`, and `created_at` expiry
//! live in `tests/sessions_exact.rs` under their `--exact` names.

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
    let session = FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog))
        .expect("store")
        .with_ttl(ttl);
    (session, dir)
}

#[test]
fn ask_refuses_empty_follow_up_and_jev_sessions_with_explicit_hints() {
    let (store, _dir) = store(Duration::from_secs(60));
    let id = store.create(BackendId::Jev, &bundle()).expect("create");
    let empty = store.ask(&id, "  ").expect_err("empty");
    assert_eq!(empty.code(), "INPUT_INVALID");
    assert!(
        empty.fix_hint().contains("non-empty"),
        "{}",
        empty.fix_hint()
    );
    let jev = store.ask(&id, "why?").expect_err("jev");
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
    let id = store.create(BackendId::Jev, &bundle()).expect("create");
    let path = store.path(&id);
    fs::write(&path, b"not-json").expect("corrupt");
    let error = store.load(&id).expect_err("parse");
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
fn session_directory_failure_names_the_directory() {
    let parent = crate::extract::test_support::temp_dir("session-file-parent");
    let blocker = parent.path().join("not-a-directory");
    fs::write(&blocker, b"x").expect("blocker");
    let nested = blocker.join("sessions");
    let error = FileSessionStore::new(nested.as_path(), Arc::new(NoopSessionLog)).expect_err("dir");
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
    let id = store.create(BackendId::Jev, &bundle()).expect("create");
    store
        .record_outcome(
            &id,
            &JudgeOutcome::new(
                BackendId::Jev,
                &JudgeRequest::new(Arc::new(bundle()), None),
                BTreeMap::new(),
            )
            .expect("outcome"),
        )
        .expect("record");
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
    assert_eq!(names, [id.to_string()]);
}

#[test]
fn temporary_names_are_unique_per_write() {
    let path = Path::new("/sessions/abc");
    let first = tmp_path(path);
    let second = tmp_path(path);
    assert_ne!(first, second);
    let name = first
        .file_name()
        .and_then(|name| name.to_str())
        .expect("name");
    assert!(name.starts_with(".abc."), "{name}");
    assert!(name.contains(&std::process::id().to_string()), "{name}");
    assert!(name.ends_with(".tmp"), "{name}");
}

#[test]
fn failed_write_removes_its_temporary_file() {
    let dir = crate::extract::test_support::temp_dir("session-tmp");
    let target = dir.path().join("occupied");
    fs::create_dir(&target).expect("directory blocks the rename");
    fs::write(target.join("keep"), b"x").expect("non-empty");
    write_private(&target, b"{}").expect_err("rename onto a directory fails");
    let leftovers: Vec<String> = fs::read_dir(dir.path())
        .expect("dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[cfg(unix)]
#[test]
fn created_session_directory_is_mode_0700() {
    use std::os::unix::fs::PermissionsExt;
    let parent = crate::extract::test_support::temp_dir("session-mode");
    let nested = parent.path().join("a").join("sessions");
    FileSessionStore::new(&nested, Arc::new(NoopSessionLog)).expect("store");
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

    let payload = utf8_payload(b"ok\xff").expect_err("not utf-8");
    assert_eq!(payload.code(), "INPUT_INVALID");
    let hint = payload.fix_hint();
    assert!(hint.contains("offset 2"), "{hint}");
    assert!(!hint.contains("schema"), "{hint}");
}
