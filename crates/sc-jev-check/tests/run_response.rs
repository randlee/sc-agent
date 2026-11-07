//! `prepare` / `run` and the `RunResponse` contract (REQ-SAG-0620,
//! REQ-SAG-0710 amended, REQ-SAG-0601 amended, REQ-SAG-0314, ADR-SAG-0024).
//!
//! Needs `--features test-util` (`RunResponse::test_builder`); the target
//! declares `required-features` (no self dev-dependency, D0 outcome 4).
//!
//! The pinned samples live in `tests/fixtures/run-response/`. Four are built
//! with `RunResponse::test_builder`; `dry_run.json` comes from a real dry
//! run. Set `SC_JEV_CHECK_BLESS=1` to rewrite them after an intended change.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::Value;

use sc_agent_types::observability::v2::NoopSessionLog;
use sc_agent_types::test_util::MockJudgeBackend;
use sc_agent_types::{
    Attestation, BackendAttestation, BackendId, BackendSet, CanonicalJudgePayload,
    EqualInputRecord, JevError, JudgeBackend, JudgeOutcome, JudgeRequest, MultiBackendMode,
    NetworkErrorKind, PayloadBundle, RepoRoot, ScAgentErrorResponse, SessionId,
};
use sc_jev_check::registry::BackendRegistry;
use sc_jev_check::run::{
    BackendFailure, BackendLimit, BackendStatus, CallerRequest, CheckSource, Envelope, RunConfig,
    RunResponse, discover_repo_root, prepare, run,
};

const SESSION_ID: &str = "3f2b8c1e-5d4a-4e6f-9a7b-0c1d2e3f4a5b";

/// The a-10 Jev response body pinned by `crates/sc-jev/tests/jev_api.rs`
/// (`ok("jev-test")`): answers carry `confidence` and `probabilities`.
fn a10_jev_body() -> Value {
    serde_json::json!({
        "model": "jev-test",
        "answers": {
            "q": {
                "choice": "no",
                "confidence": 0.9,
                "probabilities": {"yes": 0.1, "no": 0.9}
            }
        },
        "usage": {"input_tokens": 318, "output_tokens": 31}
    })
}

/// `provider_meta` shape of the API-direct LLM backends
/// (`sc-jev-backends` `wire::provider_meta`).
fn haiku_meta() -> Value {
    serde_json::json!({
        "model": "claude-haiku-test",
        "stop_reason": "end_turn",
        "status": "complete",
        "input_tokens": 412,
        "prompt_tokens": 377,
        "commands": 0
    })
}

fn payload() -> PayloadBundle {
    let payload: CanonicalJudgePayload = serde_json::from_value(serde_json::json!({
        "questions": [{"id": "q", "prompt": "Does x = 1 apply?", "choices": ["no", "yes"]}],
        "state": []
    }))
    .expect("payload");
    PayloadBundle::encode(&payload).expect("bundle")
}

fn outcome(backend: BackendId, choice: &str, meta: Value) -> JudgeOutcome {
    let request = JudgeRequest::new(Arc::new(payload()), None);
    let answers = match backend {
        BackendId::Jev => serde_json::from_value(a10_jev_body()["answers"].clone()),
        _ => serde_json::from_value(serde_json::json!({"q": {"choice": choice}})),
    }
    .expect("answers");
    JudgeOutcome::new(backend, &request, answers)
        .expect("outcome")
        .with_provider_meta(meta)
}

fn equal_input(entries: &[(BackendId, Attestation)], leak: bool) -> EqualInputRecord {
    EqualInputRecord::new(
        payload().sha256().clone(),
        MultiBackendMode::ChainContext,
        entries
            .iter()
            .map(|(id, attestation)| BackendAttestation::new(*id, attestation.clone()))
            .collect(),
        leak,
        None,
    )
    .expect("record")
}

fn session_id() -> SessionId {
    SESSION_ID.parse().expect("session id")
}

fn payload_bytes() -> u64 {
    u64::try_from(payload().bytes().len()).expect("len")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/run-response")
        .join(name)
}

/// Compares the pretty serialization of `response` to the fixture, byte for
/// byte, so key order is pinned too.
fn assert_pinned(name: &str, response: &RunResponse) -> Value {
    let mut text = serde_json::to_string_pretty(response).expect("serialize");
    text.push('\n');
    let path = fixture_path(name);
    if std::env::var_os("SC_JEV_CHECK_BLESS").is_some() {
        std::fs::write(&path, &text).expect("bless");
    }
    let pinned = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_eq!(
        text, pinned,
        "{name} drifted; re-bless only for an intended change"
    );
    serde_json::from_str(&text).expect("json")
}

/// Top-level keys of a JSON object, in order, duplicates kept.
fn top_level_keys(text: &str) -> Vec<String> {
    struct Keys(Vec<String>);
    impl<'de> serde::Deserialize<'de> for Keys {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct KeysVisitor;
            impl<'de> Visitor<'de> for KeysVisitor {
                type Value = Keys;
                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a JSON object")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Keys, A::Error> {
                    let mut keys = Vec::new();
                    while let Some(key) = map.next_key::<String>()? {
                        map.next_value::<IgnoredAny>()?;
                        keys.push(key);
                    }
                    Ok(Keys(keys))
                }
            }
            deserializer.deserialize_map(KeysVisitor)
        }
    }
    serde_json::from_str::<Keys>(text).expect("object").0
}

fn single_jev() -> RunResponse {
    RunResponse::test_builder()
        .payload_bytes(payload_bytes())
        .ok(outcome(BackendId::Jev, "no", a10_jev_body()))
        .build(equal_input(
            &[(BackendId::Jev, Attestation::Unattested)],
            false,
        ))
}

fn jev_haiku() -> RunResponse {
    RunResponse::test_builder()
        .session_saved(session_id())
        .payload_bytes(payload_bytes())
        .ok(outcome(BackendId::Jev, "no", a10_jev_body()))
        .ok(outcome(BackendId::AnthropicHaiku, "yes", haiku_meta()))
        .build(equal_input(
            &[
                (BackendId::Jev, Attestation::Unattested),
                (BackendId::AnthropicHaiku, Attestation::Unattested),
            ],
            false,
        ))
}

fn jev_failed_haiku_ok() -> RunResponse {
    RunResponse::test_builder()
        .session_saved(session_id())
        .payload_bytes(payload_bytes())
        .failed(
            BackendId::Jev,
            JevError::network(NetworkErrorKind::Connect, "connect to the Jev API failed"),
        )
        .ok(outcome(BackendId::AnthropicHaiku, "yes", haiku_meta()))
        .build(equal_input(
            &[
                (BackendId::Jev, Attestation::Failed),
                (BackendId::AnthropicHaiku, Attestation::Unattested),
            ],
            true,
        ))
}

fn session_save_failed() -> RunResponse {
    RunResponse::test_builder()
        .payload_bytes(payload_bytes())
        .ok(outcome(BackendId::AnthropicHaiku, "yes", haiku_meta()))
        .warning("session not saved: CONFIG_INVALID")
        .build(equal_input(
            &[(BackendId::AnthropicHaiku, Attestation::Unattested)],
            false,
        ))
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

fn repo(check_toml: &str, instructions: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    let questions = match instructions {
        Some(text) => QUESTIONS.replace("Does it apply?", text),
        None => QUESTIONS.to_owned(),
    };
    std::fs::write(dir.path().join("questions.json"), questions).expect("questions");
    std::fs::write(dir.path().join("check.toml"), check_toml).expect("check");
    dir
}

const PLAIN_CHECK: &str = "[questions]\npath = \"questions.json\"\n";

fn set(ids: &[BackendId]) -> BackendSet {
    BackendSet::try_from(ids.to_vec()).expect("set")
}

fn config(root: &Path) -> RunConfig {
    RunConfig::new(RepoRoot::new(root), Arc::new(NoopSessionLog))
}

fn check_file() -> CheckSource {
    CheckSource::CheckFile("check.toml".into())
}

fn real_dry_run() -> RunResponse {
    let dir = repo(PLAIN_CHECK, None);
    let prepared = prepare(
        check_file(),
        config(dir.path())
            .backends(set(&[BackendId::Jev, BackendId::AnthropicHaiku]))
            .dry_run(true),
    )
    .expect("prepare");
    run(prepared).expect("dry run")
}

#[test]
fn test_run_response_contract() {
    // single_jev.json: the Jev response verbatim at the top level.
    let single = assert_pinned("single_jev.json", &single_jev());
    let text = serde_json::to_string(&single_jev()).expect("compact");
    let keys = top_level_keys(&text);
    assert_eq!(
        keys.iter().filter(|key| *key == "answers").count(),
        1,
        "answers is written once: {keys:?}"
    );
    assert_eq!(
        keys,
        [
            "model",
            "usage",
            "answers",
            "primary_backend",
            "session_id",
            "session_saved",
            "prompt_sha256",
            "payload_bytes",
            "results",
            "equal_input"
        ]
    );
    let body = a10_jev_body();
    assert_eq!(
        serde_json::to_string(&single["answers"]).expect("answers"),
        serde_json::to_string(&body["answers"]).expect("body answers"),
        "top-level answers equal the Jev body's byte for byte"
    );
    assert_eq!(single["answers"]["q"]["confidence"], 0.9);
    assert_eq!(single["model"], body["model"]);
    assert_eq!(single["usage"], body["usage"]);
    assert_eq!(single["primary_backend"], "jev");
    assert!(single["session_id"].is_null());
    assert_eq!(single["session_saved"], false);
    assert_eq!(single["results"][0]["backend"], "jev");
    assert_eq!(single["results"][0]["status"], "ok");
    assert_eq!(single["results"][0]["outcome"]["provider_meta"], body);
    assert!(single.get("warnings").is_none());

    // jev_haiku.json: Jev primary, two ok results (REQ-SAG-0603).
    let both = assert_pinned("jev_haiku.json", &jev_haiku());
    assert_eq!(both["primary_backend"], "jev");
    assert_eq!(both["answers"]["q"]["choice"], "no");
    assert_eq!(both["session_id"], SESSION_ID);
    assert_eq!(both["session_saved"], true);
    let statuses: Vec<&Value> = both["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|result| &result["status"])
        .collect();
    assert_eq!(statuses, ["ok", "ok"]);

    // jev_failed_haiku_ok.json: Haiku primary, no Jev field at top level.
    let response = jev_failed_haiku_ok();
    assert!(response.is_partial());
    let failure = response.first_failure().expect("failure");
    assert_eq!(failure.error().code(), "NETWORK_ERROR");
    let partial = assert_pinned("jev_failed_haiku_ok.json", &response);
    assert_eq!(partial["primary_backend"], "anthropic_haiku");
    assert_eq!(partial["answers"]["q"]["choice"], "yes");
    assert_eq!(partial["model"], "claude-haiku-test");
    for jev_field in ["usage", "confidence"] {
        assert!(partial.get(jev_field).is_none(), "{jev_field} at top level");
    }
    assert!(partial["answers"]["q"].get("confidence").is_none());
    assert_eq!(partial["results"][0]["backend"], "jev");
    assert_eq!(partial["results"][0]["status"], "failed");
    let error = &partial["results"][0]["error"];
    assert_eq!(error["error"], "NETWORK_ERROR");
    assert_eq!(error["schema_version"], 1);
    assert_eq!(error["retryable"], true);
    assert!(error["message"].is_string());
    assert!(partial["results"][0].get("outcome").is_none());
    assert_eq!(partial["equal_input"]["leak"], true);

    // dry_run.json: a real dry run.
    let dry = assert_pinned("dry_run.json", &real_dry_run());
    assert!(dry.get("primary_backend").is_none());
    assert!(dry.get("answers").is_none());
    assert_eq!(dry["results"], serde_json::json!([]));
    assert!(dry["payload"]["questions"].is_array());
    assert_eq!(dry["limits"].as_array().expect("limits").len(), 2);
    assert_eq!(dry["equal_input"]["equal"], false);
    assert!(
        dry["equal_input"]["backends"]
            .as_array()
            .expect("backends")
            .iter()
            .all(|entry| entry["attestation"]["status"] == "not_sent")
    );

    // session_save_failed.json: answers kept, session not saved, one warning.
    let unsaved = assert_pinned("session_save_failed.json", &session_save_failed());
    assert_eq!(unsaved["primary_backend"], "anthropic_haiku");
    assert_eq!(unsaved["session_saved"], false);
    assert!(unsaved["session_id"].is_null());
    assert_eq!(
        unsaved["warnings"],
        serde_json::json!(["session not saved: CONFIG_INVALID"])
    );
    let keys = top_level_keys(&serde_json::to_string(&session_save_failed()).expect("text"));
    assert_eq!(keys.last().map(String::as_str), Some("warnings"));

    // REQ-SAG-0602: every sample carries the r-1 `EqualInputRecord` shape.
    for sample in [&single, &both, &partial, &dry, &unsaved] {
        let keys: Vec<&str> = sample["equal_input"]
            .as_object()
            .expect("equal_input")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "backends",
                "equal",
                "leak",
                "mode",
                "producer",
                "prompt_sha256"
            ]
        );
        assert_eq!(sample["equal_input"]["mode"], "chain_context");
        assert_eq!(
            sample["equal_input"]["prompt_sha256"],
            sample["prompt_sha256"]
        );
    }

    // The envelope wraps the same body.
    let envelope = serde_json::to_value(Envelope::run(&jev_haiku())).expect("envelope");
    assert_eq!(envelope["version"], "1");
    assert_eq!(envelope["operation"], "jev.run");
    assert_eq!(envelope["data"], both);
    let dry_envelope = Envelope::run(&real_dry_run());
    assert_eq!(dry_envelope.operation(), "jev.dry-run");
    let with_readback = Envelope::run(&single_jev())
        .with_readback(serde_json::json!({"repo_root": "/repo"}))
        .expect("readback");
    assert_eq!(with_readback.data()["readback"]["repo_root"], "/repo");
}

#[test]
fn test_backend_failure_response_derived_from_error() {
    for error in [
        JevError::network(NetworkErrorKind::Busy, "busy"),
        JevError::input_too_large(96_000, 150_000),
        JevError::from_error_response(
            "sc-treesitter",
            &serde_json::from_str::<ScAgentErrorResponse>(
                r#"{"error":"LANGUAGE_UNAVAILABLE","message":"Use python.","schema_version":1}"#,
            )
            .expect("response"),
        ),
    ] {
        let failure = BackendFailure::new(error.clone());
        assert_eq!(failure.error(), &error);
        assert_eq!(
            failure.response(),
            ScAgentErrorResponse::from_jev_error(&error)
        );
        let response = RunResponse::test_builder()
            .failed(BackendId::Jev, error.clone())
            .build(equal_input(&[(BackendId::Jev, Attestation::Failed)], true));
        let value = serde_json::to_value(&response).expect("json");
        assert_eq!(
            value["results"][0]["error"],
            serde_json::to_value(ScAgentErrorResponse::from_jev_error(&error)).expect("error")
        );
        match response.results()[0].status() {
            BackendStatus::Failed(stored) => assert_eq!(stored, &failure),
            other => panic!("expected a failure, got {other:?}"),
        }
        assert!(response.is_partial());
        assert!(response.primary().is_none());
        assert!(value.get("primary_backend").is_none());
    }
}

#[test]
fn test_backend_precedence() {
    let with_backends = repo(
        "backends = [\"anthropic_haiku\", \"jev\"]\n[questions]\npath = \"questions.json\"\n",
        None,
    );
    // 1. RunConfig::backends wins over the check.toml list.
    let flag = prepare(
        check_file(),
        config(with_backends.path()).backends(set(&[BackendId::OpenAiLuna])),
    )
    .expect("flag");
    assert_eq!(flag.backends().as_slice(), [BackendId::OpenAiLuna]);
    // 2. Else the source document's list, in its order.
    let document = prepare(check_file(), config(with_backends.path())).expect("document");
    assert_eq!(
        document.backends().as_slice(),
        [BackendId::AnthropicHaiku, BackendId::Jev]
    );
    assert!(
        document.request().llm().is_some(),
        "an LLM backend from check.toml resolves the prompt"
    );
    // 3. Else [jev].
    let plain = repo(PLAIN_CHECK, None);
    let default = prepare(check_file(), config(plain.path())).expect("default");
    assert_eq!(default.backends().as_slice(), [BackendId::Jev]);
    assert!(
        default.request().llm().is_none(),
        "Jev-only resolves no prompt"
    );

    // The same rule for a caller request document.
    let request = CallerRequest::from_json(
        br#"{"questions":[{"id":"q","prompt":"p","choices":["no","yes"]}],"backends":["openai_luna"]}"#,
    )
    .expect("request");
    let from_document = prepare(
        CheckSource::CallerRequest(request.clone()),
        config(plain.path()),
    )
    .expect("doc");
    assert_eq!(from_document.backends().as_slice(), [BackendId::OpenAiLuna]);
    let from_flag = prepare(
        CheckSource::CallerRequest(request),
        config(plain.path()).backends(set(&[BackendId::Jev])),
    )
    .expect("flag");
    assert_eq!(from_flag.backends().as_slice(), [BackendId::Jev]);
    let bare = CallerRequest::from_json(
        br#"{"questions":[{"id":"q","prompt":"p","choices":["no","yes"]}]}"#,
    )
    .expect("bare");
    let fallback =
        prepare(CheckSource::CallerRequest(bare), config(plain.path())).expect("fallback");
    assert_eq!(fallback.backends().as_slice(), [BackendId::Jev]);
}

fn judge(id: BackendId, choice: &str) -> Arc<MockJudgeBackend> {
    let answers = serde_json::from_value(serde_json::json!({"relevant": {"choice": choice}}))
        .expect("answers");
    Arc::new(MockJudgeBackend::new(id, answers))
}

#[test]
fn test_prepared_run_owns_config() {
    let dir = repo(PLAIN_CHECK, None);
    let sessions = tempfile::tempdir().expect("sessions");
    let haiku = judge(BackendId::AnthropicHaiku, "yes");
    let mut registry = BackendRegistry::new();
    registry
        .register(Arc::clone(&haiku) as Arc<dyn JudgeBackend>)
        .expect("register");
    let registry = Arc::new(registry);

    // `run` takes only the prepared run: the registry, backends, sessions
    // directory, and dry-run flag all come from the config given to
    // `prepare`.
    let prepared = prepare(
        check_file(),
        config(dir.path())
            .backends(set(&[BackendId::AnthropicHaiku]))
            .sessions_dir(sessions.path())
            .registry(Arc::clone(&registry)),
    )
    .expect("prepare");
    let digest = prepared.prompt_sha256().clone();
    let system_prompt = prepared
        .request()
        .llm()
        .map(|llm| llm.system_prompt.clone())
        .expect("prompt resolved at prepare time");
    let response = run(prepared).expect("live run");
    assert_eq!(response.prompt_sha256(), &digest);
    let requests = haiku.requests();
    assert_eq!(requests.len(), 1, "the injected registry was used");
    assert_eq!(requests[0].payload().sha256(), &digest);
    assert_eq!(
        requests[0].llm().expect("llm").system_prompt,
        system_prompt,
        "the prompt resolved by prepare is the one sent"
    );
    let id = response.session_id().expect("saved");
    assert!(sessions.path().join(format!("{id}.json")).is_file());

    let dry = prepare(
        check_file(),
        config(dir.path())
            .backends(set(&[BackendId::AnthropicHaiku]))
            .sessions_dir(sessions.path())
            .registry(registry)
            .dry_run(true),
    )
    .expect("prepare dry");
    let dry = run(dry).expect("dry");
    assert!(dry.dry_run());
    assert_eq!(haiku.requests().len(), 1, "a dry run sends nothing");
    assert_eq!(haiku.validations(), 1, "a dry run calls no backend");
}

fn git(dir: &Path, args: &[&str]) {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", null)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?}");
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("canonical")
}

#[test]
fn test_repo_root_from_subdirectory() {
    let top = tempfile::tempdir().expect("temp");
    git(top.path(), &["init", "-q"]);
    let sub = top.path().join("a").join("b");
    std::fs::create_dir_all(&sub).expect("sub");
    let from_sub = discover_repo_root(&sub).expect("root");
    assert_eq!(canonical(from_sub.as_path()), canonical(top.path()));
    assert_eq!(
        canonical(discover_repo_root(top.path()).expect("root").as_path()),
        canonical(top.path())
    );

    // A check named from the toplevel runs from a subdirectory start.
    std::fs::write(top.path().join("questions.json"), QUESTIONS).expect("questions");
    std::fs::write(top.path().join("check.toml"), PLAIN_CHECK).expect("check");
    let response = run(prepare(
        check_file(),
        RunConfig::new(from_sub, Arc::new(NoopSessionLog)).dry_run(true),
    )
    .expect("check.toml at the toplevel"))
    .expect("dry run");
    assert!(response.dry_run());

    // Outside git the start directory is the root.
    let plain = tempfile::tempdir().expect("plain");
    let outside = discover_repo_root(plain.path()).expect("outside git");
    assert!(outside.as_path().is_absolute());
    if sc_agent_types::git::discover_toplevel(plain.path())
        .expect("git runs")
        .is_none()
    {
        assert_eq!(canonical(outside.as_path()), canonical(plain.path()));
    }
}

#[test]
fn test_dry_run_reports_over_limit() {
    let limit = BackendId::Jev.descriptor().request_limit_bytes();
    let big = repo(PLAIN_CHECK, Some(&"x".repeat(150_000)));
    let backends = set(&[BackendId::Jev, BackendId::AnthropicHaiku]);
    // No registry and no keys: a dry run constructs no backend, so it
    // cannot fail on a missing key.
    let response = run(prepare(
        check_file(),
        config(big.path()).backends(backends.clone()).dry_run(true),
    )
    .expect("prepare"))
    .expect("a dry run does not fail on an over-limit payload");
    assert!(response.payload_bytes() > limit);
    let limits: BTreeMap<BackendId, &BackendLimit> = response
        .limits()
        .iter()
        .map(|entry| (entry.backend(), entry))
        .collect();
    assert_eq!(limits.len(), 2);
    for id in backends.iter() {
        let entry = limits[&id];
        assert_eq!(
            entry.request_limit_bytes(),
            id.descriptor().request_limit_bytes()
        );
        assert!(entry.over_limit(), "{id}");
    }
    let value = serde_json::to_value(&response).expect("json");
    assert_eq!(value["limits"][0]["over_limit"], true);
    assert_eq!(value["limits"][0]["request_limit_bytes"], limit);
    assert_eq!(value["payload_bytes"], response.payload_bytes());

    let small_repo = repo(PLAIN_CHECK, None);
    let small = run(prepare(
        check_file(),
        config(small_repo.path()).backends(backends).dry_run(true),
    )
    .expect("prepare"))
    .expect("small dry run");
    assert!(small.limits().iter().all(|entry| !entry.over_limit()));
}
