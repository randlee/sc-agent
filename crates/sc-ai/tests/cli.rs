//! CLI parity for `jev run`, `dry-run`, and `ask`.
//!
//! Every subprocess goes through [`spawn_with_deadline`], so a hung child
//! fails the test after 60 seconds instead of stalling CI. The runner is the
//! shared `tests/support/deadline.rs`.

#[path = "../../../tests/support/deadline.rs"]
mod deadline;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use deadline::spawn_with_deadline;
use sc_agent_types::BackendId;
use sc_agent_types::JevError;
use sc_agent_types::observability::v2::NoopSessionLog;
use sc_agent_types::test_util::MockJudgeBackend;
use sc_agent_types::{JudgeBackend, JudgeRequest};
use sc_ai::{
    ExecuteOverrides, OP_ASK, OP_DRY_RUN, OP_RUN, ObservabilitySessionLog, session_log_file,
};
use sc_jev_check::config::PACKAGED_SYSTEM_PROMPT;
use sc_jev_check::registry::BackendRegistry;
use serde_json::Value;
use tempfile::TempDir;

/// `Command::output` with [`SUBPROCESS_DEADLINE`] and no stdin.
trait RunWithDeadline {
    fn run_with_deadline(&mut self) -> Output;
}

impl RunWithDeadline for Command {
    fn run_with_deadline(&mut self) -> Output {
        spawn_with_deadline(self, None)
    }
}

fn temp_dir(label: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sc-ai-{label}-"))
        .tempdir()
        .expect("temp")
}

fn judge(id: BackendId, choice: &str) -> MockJudgeBackend {
    let answers = serde_json::from_value(serde_json::json!({"relevant": {"choice": choice}}))
        .expect("answers");
    MockJudgeBackend::new(id, answers)
}

/// Requests recorded by the judges attached to this log, in judge order.
#[derive(Clone, Default)]
struct RequestLog(Arc<Mutex<Vec<Arc<MockJudgeBackend>>>>);

impl RequestLog {
    fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> Vec<JudgeRequest> {
        self.0
            .lock()
            .expect("log")
            .iter()
            .flat_map(|judge| judge.requests())
            .collect()
    }
}

/// Keeps a handle on a judge so the test can read what it received.
trait WithRequestLog {
    fn with_request_log(self, log: &RequestLog) -> Arc<MockJudgeBackend>;
}

impl WithRequestLog for MockJudgeBackend {
    fn with_request_log(self, log: &RequestLog) -> Arc<MockJudgeBackend> {
        let judge = Arc::new(self);
        log.0.lock().expect("log").push(Arc::clone(&judge));
        judge
    }
}

fn registry<T: Into<Arc<MockJudgeBackend>>>(judges: Vec<T>) -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    for judge in judges {
        registry
            .register(judge.into() as Arc<dyn JudgeBackend>)
            .expect("register");
    }
    registry
}

fn write_repo(dir: &Path) {
    std::fs::write(
        dir.join("questions.json"),
        r#"{
  "schema": "plan-anchoring/questions/1",
  "version": 1,
  "questions": {
    "relevant": {
      "type": "choice",
      "instructions": "Does it apply?",
      "criteria": {"no": "No.", "yes": "Yes."}
    }
  }
}"#,
    )
    .expect("questions");
    std::fs::write(
        dir.join("check.toml"),
        "[questions]\npath = \"questions.json\"\n",
    )
    .expect("check");
}

fn noop_overrides(registry: BackendRegistry) -> ExecuteOverrides {
    ExecuteOverrides {
        registry: Some(registry),
        session_log: Some(Arc::new(NoopSessionLog)),
        ..ExecuteOverrides::default()
    }
}

fn execute(args: &[&str], overrides: ExecuteOverrides) -> Result<Value, JevError> {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let bytes = sc_ai::execute_with_overrides(&args, &[], overrides)?;
    serde_json::from_slice(&bytes).map_err(|_| {
        JevError::input_invalid(format!(
            "stdout was not JSON: {}",
            String::from_utf8_lossy(&bytes)
        ))
    })
}

fn canon(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn assert_no_prompt_leak(text: &str) {
    let packaged_first_line = PACKAGED_SYSTEM_PROMPT.lines().next().expect("prompt line");
    assert!(
        !text.contains(packaged_first_line),
        "packaged prompt leaked into {text}"
    );
    for token in [
        "OVERRIDE-PROMPT-TOKEN-a7",
        "EVALUATOR-PROMPT-a7",
        "sk-ant-test-secret",
        "sk-test-whitespace",
    ] {
        assert!(!text.contains(token), "{token} leaked into {text}");
    }
}

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sc-ai")
}

#[test]
fn test_multi_backend_cli_equal_input() {
    let repo = temp_dir("multi");
    write_repo(repo.path());
    let sessions = temp_dir("multi-sessions");
    let jev_seen = RequestLog::new();
    let haiku_seen = RequestLog::new();
    let value = execute(
        &[
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--check",
            "check.toml",
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--backend",
            "jev",
            "--backend",
            "anthropic_haiku",
        ],
        noop_overrides(registry(vec![
            judge(BackendId::Jev, "yes").with_request_log(&jev_seen),
            judge(BackendId::AnthropicHaiku, "no").with_request_log(&haiku_seen),
        ])),
    )
    .expect("run");
    assert_eq!(value["version"], "1");
    assert_eq!(value["operation"], OP_RUN);
    let data = &value["data"];
    assert!(data["session_id"].as_str().is_some());
    assert_eq!(data["answers"]["relevant"]["choice"], "yes");
    assert_eq!(data["outcomes"][0]["backend"], "jev");
    assert!(data.get("backend").is_none());
    assert!(data.get("provider_meta").is_none());
    assert!(data.get("probabilities").is_none());
    assert_eq!(data["outcomes"].as_array().expect("outcomes").len(), 2);
    // r-1: no backend reports an input digest yet, so each is `unattested`
    // and the derived `equal` is false (backend-side hashing lands in r-2b).
    assert_eq!(data["equal_input"]["equal"], false);
    assert_eq!(data["equal_input"]["leak"], false);
    assert_eq!(data["equal_input"]["mode"], "chain_context");
    assert_eq!(data["equal_input"]["prompt_sha256"], data["prompt_sha256"]);
    assert_eq!(
        data["equal_input"]["backends"],
        serde_json::json!([
            {"backend": "jev", "attestation": {"status": "unattested"}},
            {"backend": "anthropic_haiku", "attestation": {"status": "unattested"}}
        ])
    );
    let jev = jev_seen.snapshot();
    let haiku = haiku_seen.snapshot();
    assert_eq!(jev.len(), 1);
    assert_eq!(haiku.len(), 1);
    assert_eq!(jev[0].payload().bytes(), haiku[0].payload().bytes());
    assert_eq!(jev[0].payload().sha256().as_str(), data["prompt_sha256"]);
    assert!(jev[0].llm().is_none());
    let prompt = &haiku[0].llm().as_ref().expect("haiku prompt").system_prompt;
    assert!(prompt.starts_with(PACKAGED_SYSTEM_PROMPT));
    assert!(prompt.contains("Session context:"));
    let payload = String::from_utf8_lossy(jev[0].payload().bytes());
    assert!(!payload.contains("Session context:"));
    assert!(!payload.contains(PACKAGED_SYSTEM_PROMPT.trim()));
    assert_no_prompt_leak(&value.to_string());
}

#[test]
fn test_cli_prompt_override() {
    let repo = temp_dir("prompt");
    write_repo(repo.path());
    let override_path = repo.path().join("override.md");
    std::fs::write(&override_path, "OVERRIDE-PROMPT-TOKEN-a7\n").expect("override");
    let first_seen = RequestLog::new();
    let first = execute(
        &[
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            temp_dir("prompt-sessions-a").path().to_str().expect("utf8"),
            "--backend",
            "anthropic_haiku",
        ],
        noop_overrides(registry(vec![
            judge(BackendId::AnthropicHaiku, "yes").with_request_log(&first_seen),
        ])),
    )
    .expect("packaged");
    let second_seen = RequestLog::new();
    let sessions = temp_dir("prompt-sessions-b");
    let second = execute(
        &[
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--backend",
            "anthropic_haiku",
            "--llm-system-prompt-file",
            override_path.to_str().expect("utf8"),
        ],
        noop_overrides(registry(vec![
            judge(BackendId::AnthropicHaiku, "no").with_request_log(&second_seen),
        ])),
    )
    .expect("override");
    assert_eq!(
        first["data"]["prompt_sha256"],
        second["data"]["prompt_sha256"]
    );
    let packaged = first_seen.snapshot()[0]
        .llm()
        .as_ref()
        .expect("prompt")
        .system_prompt
        .clone();
    let overridden = second_seen.snapshot()[0]
        .llm()
        .as_ref()
        .expect("prompt")
        .system_prompt
        .clone();
    assert_eq!(packaged, PACKAGED_SYSTEM_PROMPT);
    assert!(overridden.contains("OVERRIDE-PROMPT-TOKEN-a7"));
    assert!(!overridden.contains(PACKAGED_SYSTEM_PROMPT.trim()));
    assert_eq!(
        second["data"]["readback"]["llm_system_prompt_file"],
        canon(&override_path)
    );
    assert_eq!(second["data"]["readback"]["repo_root"], canon(repo.path()));
    assert_eq!(
        second["data"]["readback"]["check_toml"],
        canon(&repo.path().join("check.toml"))
    );
    assert_no_prompt_leak(&first.to_string());
    assert_no_prompt_leak(&second.to_string());
}

#[test]
fn cli_dry_run_readback_and_ask_session_ttl() {
    let repo = temp_dir("ask");
    write_repo(repo.path());
    let sessions = temp_dir("ask-sessions");
    let prompt = repo.path().join("prompt.md");
    std::fs::write(&prompt, "EVALUATOR-PROMPT-a7\n").expect("prompt");
    let seen = RequestLog::new();
    let now = Arc::new(Mutex::new(UNIX_EPOCH + Duration::from_secs(1_700_000_000)));
    let clock_now = Arc::clone(&now);
    let overrides = ExecuteOverrides {
        registry: Some(registry(vec![
            judge(BackendId::AnthropicHaiku, "yes").with_request_log(&seen),
        ])),
        clock: Some(Arc::new(move || {
            *clock_now
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })),
        session_ttl: Some(Duration::from_secs(60)),
        session_log: Some(Arc::new(NoopSessionLog)),
    };
    let dry = execute(
        &[
            "jev",
            "dry-run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--check",
            "check.toml",
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--backend",
            "anthropic_haiku",
            "--llm-system-prompt-file",
            prompt.to_str().expect("utf8"),
        ],
        ExecuteOverrides {
            registry: overrides.registry.clone(),
            session_log: Some(Arc::new(NoopSessionLog)),
            ..ExecuteOverrides::default()
        },
    )
    .expect("dry-run");
    assert_eq!(dry["version"], "1");
    assert_eq!(dry["operation"], OP_DRY_RUN);
    assert!(dry["data"]["session_id"].is_null());
    assert_eq!(
        dry["data"]["prompt_sha256"].as_str().expect("sha").len(),
        64
    );
    assert!(dry["data"]["payload"]["questions"].is_array());
    assert_eq!(dry["data"]["readback"]["repo_root"], canon(repo.path()));
    assert_eq!(
        dry["data"]["readback"]["check_toml"],
        canon(&repo.path().join("check.toml"))
    );
    assert_eq!(
        dry["data"]["readback"]["llm_system_prompt_file"],
        canon(&prompt)
    );
    assert!(dry["data"]["readback"]["request_json"].is_null());
    assert!(seen.snapshot().is_empty());
    assert_no_prompt_leak(&dry.to_string());

    let ran = execute(
        &[
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--backend",
            "anthropic_haiku",
            "--llm-system-prompt-file",
            prompt.to_str().expect("utf8"),
        ],
        ExecuteOverrides {
            registry: overrides.registry.clone(),
            clock: overrides.clock.clone(),
            session_ttl: overrides.session_ttl,
            session_log: Some(Arc::new(NoopSessionLog)),
        },
    )
    .expect("run");
    assert_eq!(ran["operation"], OP_RUN);
    let session_id = ran["data"]["session_id"]
        .as_str()
        .expect("session")
        .to_owned();
    assert_eq!(ran["data"]["prompt_sha256"], dry["data"]["prompt_sha256"]);
    let wire = seen.snapshot()[0]
        .llm()
        .as_ref()
        .expect("wire")
        .system_prompt
        .clone();
    assert!(wire.contains("EVALUATOR-PROMPT-a7"));
    assert!(
        !String::from_utf8_lossy(seen.snapshot()[0].payload().bytes())
            .contains("EVALUATOR-PROMPT-a7")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(sessions.path().join(&session_id))
            .expect("session file")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    let asked = execute(
        &[
            "jev",
            "ask",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--session-id",
            &session_id,
            "--text",
            "why the choice?",
        ],
        ExecuteOverrides {
            registry: overrides.registry.clone(),
            clock: overrides.clock.clone(),
            session_ttl: overrides.session_ttl,
            session_log: Some(Arc::new(NoopSessionLog)),
        },
    )
    .expect("ask");
    assert_eq!(asked["version"], "1");
    assert_eq!(asked["operation"], OP_ASK);
    assert_eq!(asked["data"]["session_id"], session_id);
    assert!(asked["data"].get("backend").is_none());
    assert!(asked["data"].get("provider_meta").is_none());
    assert_eq!(asked["data"]["answers"]["relevant"]["choice"], "yes");
    assert_eq!(
        asked["data"]["readback"]["sessions_dir"],
        canon(sessions.path())
    );
    assert_eq!(asked["data"]["prompt_sha256"], ran["data"]["prompt_sha256"]);
    let follow = seen.snapshot()[1]
        .llm()
        .as_ref()
        .expect("follow")
        .system_prompt
        .clone();
    assert!(follow.contains("why the choice?"));
    assert_no_prompt_leak(&asked.to_string());

    *now.lock().expect("clock") += Duration::from_secs(120);
    let expired = sc_ai::execute_with_overrides(
        &[
            "jev".into(),
            "ask".into(),
            "--repo-root".into(),
            repo.path().display().to_string(),
            "--sessions-dir".into(),
            sessions.path().display().to_string(),
            "--session-id".into(),
            session_id.clone(),
            "--text".into(),
            "still there?".into(),
        ],
        &[],
        ExecuteOverrides {
            registry: overrides.registry,
            clock: overrides.clock,
            session_ttl: overrides.session_ttl,
            session_log: Some(Arc::new(NoopSessionLog)),
        },
    )
    .expect_err("expired");
    assert_eq!(expired.code(), "SESSION_EXPIRED");
    assert!(expired.fix_hint().contains(&session_id));
    assert_no_prompt_leak(&expired.fix_hint());
}

#[test]
fn cli_logger_adapter_records_session_without_secrets() {
    let repo = temp_dir("log");
    write_repo(repo.path());
    let sessions = temp_dir("log-sessions");
    let logs = temp_dir("log-root");
    let prompt = repo.path().join("prompt.md");
    std::fs::write(&prompt, "EVALUATOR-PROMPT-a7 sk-ant-test-secret\n").expect("prompt");
    let seen = RequestLog::new();
    let value = execute(
        &[
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--log-dir",
            logs.path().to_str().expect("utf8"),
            "--backend",
            "anthropic_haiku",
            "--llm-system-prompt-file",
            prompt.to_str().expect("utf8"),
        ],
        ExecuteOverrides {
            registry: Some(registry(vec![
                judge(BackendId::AnthropicHaiku, "yes").with_request_log(&seen),
            ])),
            ..ExecuteOverrides::default()
        },
    )
    .expect("logged run");
    let digest = value["data"]["prompt_sha256"].as_str().expect("sha");
    let session_id = value["data"]["session_id"].as_str().expect("session");
    let log_path = session_log_file(logs.path());
    let log = std::fs::read_to_string(&log_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", log_path.display()));
    assert!(log.contains(session_id), "{log}");
    assert!(log.contains(digest), "{log}");
    assert!(log.contains("judge"), "{log}");
    assert!(log.contains("anthropic_haiku"), "{log}");
    assert_no_prompt_leak(&log);
    assert_no_prompt_leak(&value.to_string());
    assert!(
        seen.snapshot()[0]
            .llm()
            .as_ref()
            .expect("prompt")
            .system_prompt
            .contains("sk-ant-test-secret")
    );
}

#[test]
fn cli_invalid_request_json_is_input_invalid() {
    let error = sc_ai::execute_with_overrides(
        &[
            "jev".into(),
            "run".into(),
            "--request-json".into(),
            "-".into(),
        ],
        br#"{"invalid":true}"#,
        ExecuteOverrides::default(),
    )
    .expect_err("invalid");
    assert_eq!(error.code(), "INPUT_INVALID");
    assert!(!error.fix_hint().contains("invalid"));
}

#[test]
fn cli_request_json_rejects_inline_system_prompt() {
    let error = sc_ai::execute_with_overrides(
        &[
            "jev".into(),
            "run".into(),
            "--request-json".into(),
            "-".into(),
        ],
        br#"{"questions":[{"id":"q","prompt":"p","choices":["a","b"]}],"system_prompt":"INLINE-PROMPT-TOKEN"}"#,
        ExecuteOverrides::default(),
    )
    .expect_err("inline");
    assert_eq!(error.code(), "INPUT_INVALID");
    let hint = error.fix_hint();
    assert!(hint.contains("--llm-system-prompt-file"));
    assert!(hint.contains("packaged default"));
    assert!(!hint.contains("INLINE-PROMPT-TOKEN"));
}

#[test]
fn cli_keeps_typesafe_response_fields() {
    let repo = temp_dir("verbatim");
    write_repo(repo.path());
    let sessions = temp_dir("verbatim-sessions");
    let seen = RequestLog::new();
    let usage = serde_json::json!({"input_tokens": 318, "output_tokens": 31});
    let backend = judge(BackendId::Jev, "yes")
        .with_provider_meta(serde_json::json!({
            "model": "jev-test",
            "usage": usage,
            "stop_reason": "end_turn",
            "id": "resp-1"
        }))
        .with_request_log(&seen);
    let value = execute(
        &[
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--backend",
            "jev",
        ],
        noop_overrides(registry(vec![backend])),
    )
    .expect("run");
    let data = &value["data"];
    assert_eq!(data["model"], "jev-test");
    assert_eq!(data["usage"], usage);
    assert_eq!(data["stop_reason"], "end_turn");
    assert_eq!(data["id"], "resp-1");
    assert!(data["session_id"].as_str().is_some());
    assert_eq!(data["answers"]["relevant"]["choice"], "yes");
    assert!(data.get("provider_meta").is_none());
    assert!(data.get("backend").is_none());
}

#[test]
fn cli_log_and_git_errors_do_not_blame_check_toml() {
    let root = temp_dir("logblock");
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, b"x").expect("file");
    let error = match ObservabilitySessionLog::open(&file.join("nested")) {
        Ok(_) => panic!("log directory should fail when a parent path is a file"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "CONFIG_INVALID");
    let hint = error.fix_hint();
    assert!(!hint.contains("check.toml"), "{hint}");
    assert!(hint.contains("--log-dir"), "{hint}");

    let outside = temp_dir("nogit");
    let output = Command::new(binary())
        .args(["jev", "dry-run"])
        .current_dir(outside.path())
        .env("GIT_DIR", outside.path().join("missing-git"))
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_TERMINAL_PROMPT", "0")
        .run_with_deadline();
    assert_eq!(output.status.code(), Some(2));
    let err: Value = serde_json::from_slice(&output.stderr).expect("stderr json");
    assert_eq!(err["error"], "CONFIG_INVALID");
    let message = err["message"].as_str().expect("message");
    assert!(message.contains("--repo-root"), "{message}");
    assert!(!message.contains("check.toml"), "{message}");
    assert!(!message.contains("schema"), "{message}");
}

#[test]
fn cli_process_version_help_and_typed_stderr() {
    let version = Command::new(binary()).arg("--version").run_with_deadline();
    assert!(version.status.success());
    assert!(version.stderr.is_empty());
    let stdout = String::from_utf8(version.stdout).expect("utf8");
    assert!(stdout.starts_with("sc-ai "));

    let help = Command::new(binary())
        .args(["jev", "dry-run", "--help"])
        .run_with_deadline();
    assert!(help.status.success(), "help status {:?}", help.status);
    assert!(help.stderr.is_empty());
    let help_text = String::from_utf8(help.stdout).expect("utf8");
    assert!(help_text.contains("--llm-system-prompt-file"));
    assert!(help_text.contains("--repo-root"));
    assert!(help_text.contains("SC_LOG_ROOT"));
    assert!(help_text.contains(".sc-ai"));

    let output = spawn_with_deadline(
        Command::new(binary()).args(["jev", "run", "--request-json", "-"]),
        Some(br#"{"invalid":true}"#),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    // REQ-SAG-0701 (r-1): every stderr line is one `ScAgentErrorResponse`
    // with `schema_version: 1`.
    let stderr = String::from_utf8(output.stderr.clone()).expect("utf8 stderr");
    for line in stderr.lines() {
        let parsed: sc_agent_types::ScAgentErrorResponse =
            serde_json::from_str(line).expect("stderr line is an error response");
        assert_eq!(parsed.schema_version(), 1);
    }
    let err: Value = serde_json::from_slice(&output.stderr).expect("stderr json");
    let object = err.as_object().expect("object");
    assert_eq!(object.len(), 3);
    assert_eq!(err["error"], "INPUT_INVALID");
    assert_eq!(err["schema_version"], 1);
    assert!(err["message"].is_string());
    let codes: Value = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/jev/README-contract/errors.json"),
        )
        .expect("errors.json"),
    )
    .expect("codes");
    let ids = codes["error_ids"].as_array().expect("ids");
    assert!(ids.iter().any(|id| id == "INPUT_INVALID"));
}

#[test]
fn cli_run_resolves_dotenv_key_and_dry_run_does_not() {
    let repo = temp_dir("dotenv");
    write_repo(repo.path());
    std::fs::write(
        repo.path().join(".env"),
        "TYPESAFE_API_KEY=sk-test-whitespace key\n",
    )
    .expect("env");
    let sessions = temp_dir("dotenv-sessions");
    let logs = temp_dir("dotenv-logs");
    let dry = Command::new(binary())
        .args([
            "jev",
            "dry-run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--log-dir",
            logs.path().to_str().expect("utf8"),
            "--backend",
            "jev",
        ])
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("SC_LOG_ROOT")
        .run_with_deadline();
    assert!(
        dry.status.success(),
        "dry-run stderr {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(dry.stderr.is_empty());
    let dry_json: Value = serde_json::from_slice(&dry.stdout).expect("dry json");
    assert_eq!(dry_json["operation"], OP_DRY_RUN);
    assert!(dry_json["data"]["session_id"].is_null());
    assert_no_prompt_leak(&String::from_utf8_lossy(&dry.stdout));

    let run = Command::new(binary())
        .args([
            "jev",
            "run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.path().to_str().expect("utf8"),
            "--log-dir",
            logs.path().to_str().expect("utf8"),
            "--backend",
            "jev",
        ])
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("SC_LOG_ROOT")
        .run_with_deadline();
    assert_eq!(run.status.code(), Some(2), "run should refuse the bad key");
    assert!(run.stdout.is_empty());
    let err: Value = serde_json::from_slice(&run.stderr).expect("run stderr");
    assert_eq!(err["error"], "CONFIG_INVALID");
    let rendered = format!("{} {}", err["error"], err["message"]);
    assert_no_prompt_leak(&rendered);
    assert!(
        !rendered.to_ascii_lowercase().contains("bearer"),
        "{rendered}"
    );
}

#[test]
fn cli_dry_run_writes_nothing_and_ask_uses_default_log_root() {
    let repo = temp_dir("logroot");
    write_repo(repo.path());
    let sessions_parent = temp_dir("logroot-sessions");
    let sessions = sessions_parent.path().join("sessions");
    let dry = Command::new(binary())
        .args([
            "jev",
            "dry-run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.to_str().expect("utf8"),
        ])
        .env_remove("SC_LOG_ROOT")
        .run_with_deadline();
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(
        !repo.path().join(".sc-ai").exists(),
        "dry-run made a log root"
    );
    assert!(!sessions.exists(), "dry-run made a sessions dir");

    let absent = "00000000-0000-4000-8000-000000000000";
    let ask = |log_root: Option<&Path>| {
        let mut command = Command::new(binary());
        command.args([
            "jev",
            "ask",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--sessions-dir",
            sessions.to_str().expect("utf8"),
            "--session-id",
            absent,
            "--text",
            "why?",
        ]);
        match log_root {
            Some(root) => command.env("SC_LOG_ROOT", root),
            None => command.env_remove("SC_LOG_ROOT"),
        };
        command.run_with_deadline()
    };
    let default_root = ask(None);
    assert_eq!(default_root.status.code(), Some(2));
    let err: Value = serde_json::from_slice(&default_root.stderr).expect("stderr json");
    assert_eq!(err["error"], "SESSION_NOT_FOUND");
    assert!(repo.path().join(".sc-ai").is_dir());

    let custom = temp_dir("logroot-env");
    let env_root = custom.path().join("root");
    let env_run = ask(Some(&env_root));
    assert_eq!(env_run.status.code(), Some(2));
    assert!(env_root.is_dir());
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// Git for test fixtures: no global config, no prompt, fixed identity.
fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.email=sc-ai@example.com",
            "-c",
            "user.name=sc-ai",
        ])
        .args(args)
        .run_with_deadline();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_check_that_exists_only_at_rev_loads_with_rev() {
    let repo = temp_dir("rev-only");
    write_repo(repo.path());
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-q", "-m", "init"]);
    std::fs::remove_file(repo.path().join("check.toml")).expect("remove worktree copy");
    let value = execute(
        &[
            "jev",
            "dry-run",
            "--repo-root",
            repo.path().to_str().expect("utf8"),
            "--check",
            "check.toml",
            "--rev",
            "HEAD",
        ],
        noop_overrides(BackendRegistry::new()),
    )
    .expect("check.toml at HEAD");
    assert_eq!(value["operation"], OP_DRY_RUN);
    assert!(value["data"]["payload"]["questions"].is_array());
}
