//! Session document versioning (r-1 D7.6, REQ-SAG-0600).
//!
//! Version 2 is the only accepted session document. A Phase A (version 1)
//! file and a file written by a newer sc-agent both load as
//! `SessionDocumentInvalid` (`INPUT_INVALID`) with their own recovery text.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sc_agent_types::observability::v2::NoopSessionLog;
use sc_agent_types::{JevError, SessionId};
use sc_jev_check::session::FileSessionStore;

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
    FileSessionStore::new(dir.path(), Arc::new(NoopSessionLog))
        .expect("store")
        .with_clock(|| SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_001_000))
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
