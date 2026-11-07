//! Request byte cap owned by each backend (REQ-SAG-0314, ADR-SAG-0004, D13).
//!
//! The engine no longer refuses an oversize payload itself (the Phase A cap
//! was `max_input_bytes = 200_000` in packaged `jev.toml`). The production
//! backends refuse a payload above their descriptor limit (96 000 bytes) in
//! `validate_for_send`, which `run` calls on every backend before the first
//! send. Real backends are built with `BackendRegistry::from_resolver` and a
//! placeholder key: a refused request never reaches the transport.

use std::sync::Arc;

use sc_agent_types::observability::v2::NoopSessionLog;
use sc_agent_types::{BackendId, BackendSet, ErrorCode, JevError, RepoRoot, ResolvedApiKey};
use sc_jev_check::registry::BackendRegistry;
use sc_jev_check::run::{CheckSource, RunConfig, prepare, run};

/// Phase A engine cap that no longer exists.
const OLD_ENGINE_CAP: u64 = 200_000;

/// A questions document whose canonical payload is about `target` bytes.
fn repo_with_payload_near(target: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    let instructions = "x".repeat(target);
    let questions = serde_json::json!({
        "schema": "plan-anchoring/questions/1",
        "version": 1,
        "questions": {
            "relevant": {
                "type": "choice",
                "instructions": instructions,
                "criteria": {"no": "No.", "yes": "Yes."}
            }
        }
    });
    std::fs::write(dir.path().join("questions.json"), questions.to_string()).expect("questions");
    std::fs::write(
        dir.path().join("check.toml"),
        "[questions]\npath = \"questions.json\"\n",
    )
    .expect("check");
    dir
}

#[test]
fn test_payload_between_old_caps_refused_by_backend_before_send() {
    let limit = BackendId::Jev.descriptor().request_limit_bytes();
    let dir = repo_with_payload_near(150_000);
    let sessions = tempfile::tempdir().expect("sessions");
    for backend in [
        BackendId::Jev,
        BackendId::AnthropicHaiku,
        BackendId::OpenAiLuna,
    ] {
        assert_eq!(backend.descriptor().request_limit_bytes(), limit);
        let backends = BackendSet::try_from(vec![backend]).expect("set");
        let registry = BackendRegistry::from_resolver(&backends, |key_id| {
            ResolvedApiKey::try_new(key_id, "placeholder-key-never-sent".to_owned())
        })
        .expect("production backends");
        let config = RunConfig::new(RepoRoot::new(dir.path()), Arc::new(NoopSessionLog))
            .backends(backends)
            .sessions_dir(sessions.path())
            .registry(Arc::new(registry));
        let prepared = prepare(CheckSource::CheckFile("check.toml".into()), config)
            .expect("the engine builds the payload without a cap");
        let payload_bytes = prepared.payload_bytes();
        assert!(
            payload_bytes > limit && payload_bytes < OLD_ENGINE_CAP,
            "payload {payload_bytes} sits between the backend limit {limit} and the old cap"
        );
        let error = run(prepared).expect_err("the backend refuses before any send");
        assert_eq!(
            error.code(),
            ErrorCode::InputTooLarge,
            "{backend}: {error:?}"
        );
        match error {
            JevError::InputTooLarge {
                limit_bytes,
                actual_bytes,
                ..
            } => {
                assert_eq!(limit_bytes, limit, "{backend}");
                assert!(actual_bytes > limit, "{backend}");
            }
            other => panic!("{backend}: unexpected {other:?}"),
        }
        assert_eq!(
            std::fs::read_dir(sessions.path()).expect("dir").count(),
            0,
            "{backend}: no session after a refusal"
        );
    }
}
