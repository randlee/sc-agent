//! `build_request`, `run_check`, and session `ask`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBytes};
use sc_agent_types::{
    BackendId, EqualInputRecord, JevError, JudgeOutcome, RepoRoot, Rev, SessionId,
    SubjectsProducer, canonical_json_bytes,
};
use sc_jev_check::config::load_check_at;
use sc_jev_check::equal_input::producer_from_extractions;
use sc_jev_check::payload::PayloadBuilder;
use sc_jev_check::run::{RunOptions, run_check as run_check_facade};
use sc_jev_check::session::{FileSessionStore, SessionStore};
use serde_json::Value;

use crate::error::{self, map_err};
use crate::logging::{self, LoggerGuard};
use crate::scripted;

/// One Python `run_check` invocation.
struct CheckRun {
    check_path: String,
    backends: Vec<String>,
    dry_run: bool,
    repo_root: Option<PathBuf>,
    rev: String,
    log_dir: Option<PathBuf>,
    session_dir: Option<PathBuf>,
    system_prompt_file: Option<PathBuf>,
    scripted_answers: Option<Value>,
}

/// Facade result kept alive so `ask` reuses the injected backends and log.
struct FinishedRun {
    session_id: Option<SessionId>,
    payload: String,
    prompt_sha256: String,
    outcomes: Vec<JudgeOutcome>,
    equal_input: Option<EqualInputRecord>,
    system_prompt: Option<String>,
    store: FileSessionStore,
    logger: LoggerGuard,
}

struct BuiltRequest {
    payload: String,
    prompt_sha256: String,
    producer: Option<SubjectsProducer>,
}

/// Canonical JSON bytes for `json_text` (ADR-SAG-0002).
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when `json_text` is not JSON or cannot
/// be encoded.
pub(crate) fn canonical_of(json_text: &str) -> Result<Vec<u8>, JevError> {
    let value: Value = serde_json::from_str(json_text).map_err(|error| JevError::InputInvalid {
        detail: format!("JSON text: {error}"),
    })?;
    canonical_json_bytes(&value)
}

fn execute(spec: CheckRun) -> Result<FinishedRun, JevError> {
    let backends = parse_backends(&spec.backends)?;
    let repo = resolve_repo(spec.repo_root.as_deref())?;
    let rev = parse_rev(&spec.rev);
    let check = load_check_at(&repo, &rev, &spec.check_path)?;
    let scripted_answers = spec.scripted_answers.map(normalize_answers).transpose()?;
    let log_root = logging::resolve_log_root(spec.log_dir.as_deref());
    let session_dir = logging::resolve_session_dir(spec.session_dir.as_deref(), &log_root);
    let (logger, session_log) = logging::open_session_log(&log_root)?;
    let store = FileSessionStore::new(&session_dir, Arc::clone(&session_log))?;
    let mut options = RunOptions::new(repo, store.clone(), session_log)
        .dry_run(spec.dry_run)
        .rev(rev);
    if let Some(path) = spec.system_prompt_file {
        options = options.system_prompt_override(path);
    }
    if let Some(answers) = scripted_answers {
        options = options.registry(scripted::registry(&backends, answers)?);
    }
    let result = run_check_facade(&check, &backends, options)?;
    logger.flush();
    let payload = utf8_payload(result.bundle.bytes)?;
    Ok(FinishedRun {
        session_id: result.session_id,
        prompt_sha256: result.bundle.sha256.to_string(),
        payload,
        outcomes: result.outcomes,
        equal_input: result.equal_input,
        system_prompt: result.system_prompt,
        store,
        logger,
    })
}

fn build(
    check_path: &str,
    repo_root: Option<&Path>,
    rev_text: &str,
) -> Result<BuiltRequest, JevError> {
    let repo = resolve_repo(repo_root)?;
    let rev = parse_rev(rev_text);
    let spec = load_check_at(&repo, &rev, check_path)?;
    let built = PayloadBuilder::packaged()?.build(&spec, &repo, &rev)?;
    let producer = producer_from_extractions(&built.extractions);
    Ok(BuiltRequest {
        payload: utf8_payload(built.bundle.bytes)?,
        prompt_sha256: built.bundle.sha256.to_string(),
        producer,
    })
}

fn utf8_payload(bytes: Vec<u8>) -> Result<String, JevError> {
    String::from_utf8(bytes).map_err(|_| JevError::InputInvalid {
        detail: "canonical payload is not UTF-8".into(),
    })
}

fn parse_backends(names: &[String]) -> Result<Vec<BackendId>, JevError> {
    if names.is_empty() {
        return Err(JevError::ConfigInvalid {
            detail: "at least one backend is required".into(),
        });
    }
    names.iter().map(|name| parse_backend(name)).collect()
}

fn parse_backend(text: &str) -> Result<BackendId, JevError> {
    match text {
        "jev" => Ok(BackendId::Jev),
        "anthropic_haiku" | "anthropic-haiku" => Ok(BackendId::AnthropicHaiku),
        "openai_luna" | "openai-luna" => Ok(BackendId::OpenAiLuna),
        other => Err(JevError::registry_invalid(&format!(
            "Backend {other} is not registered. Use jev, anthropic_haiku, or openai_luna."
        ))),
    }
}

fn parse_rev(text: &str) -> Rev {
    if text.is_empty() || text == Rev::WORKTREE {
        Rev::worktree()
    } else {
        Rev::new(text)
    }
}

fn resolve_repo(repo_root: Option<&Path>) -> Result<RepoRoot, JevError> {
    match repo_root.filter(|path| !path.as_os_str().is_empty()) {
        Some(path) => Ok(RepoRoot::new(path)),
        None => {
            let current = std::env::current_dir().map_err(|error| JevError::ConfigInvalid {
                detail: format!("current directory is unavailable ({error})"),
            })?;
            Ok(RepoRoot::new(current))
        }
    }
}

fn normalize_answers(value: Value) -> Result<Value, JevError> {
    let Value::Object(map) = value else {
        return Err(JevError::InputInvalid {
            detail: "scripted_answers must be a JSON object of question ids".into(),
        });
    };
    let mut answers = serde_json::Map::new();
    for (id, answer) in map {
        let wrapped = match answer {
            Value::String(choice) => serde_json::json!({ "choice": choice }),
            other @ Value::Object(_) => other,
            _ => {
                return Err(JevError::InputInvalid {
                    detail: format!("scripted answer for {id} must be a string or object"),
                });
            }
        };
        answers.insert(id, wrapped);
    }
    Ok(Value::Object(answers))
}

fn optional_path(value: Option<&str>) -> Option<PathBuf> {
    value.filter(|text| !text.is_empty()).map(PathBuf::from)
}

fn py_to_value(obj: &Bound<'_, PyAny>) -> Result<Value, JevError> {
    let text = python_json(obj).map_err(|error| JevError::InputInvalid {
        detail: format!("scripted_answers is not JSON: {error}"),
    })?;
    serde_json::from_str(&text).map_err(|error| JevError::InputInvalid {
        detail: format!("scripted_answers is not JSON: {error}"),
    })
}

fn python_json(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    let json = obj.py().import("json")?;
    json.call_method1("dumps", (obj,))?.extract()
}

/// Maps a `serde_json` encode failure onto [`JevError::InputInvalid`].
///
/// Callers pass the result to [`error::to_py_err`]. `label` names the value
/// that failed. The detail carries the serde message and no secret material.
fn encode_failure(label: &str, error: &serde_json::Error) -> JevError {
    JevError::InputInvalid {
        detail: format!("{label}: {error}"),
    }
}

fn encode_py_err(py: Python<'_>, label: &str, error: serde_json::Error) -> PyErr {
    error::to_py_err(py, &encode_failure(label, &error))
}

fn to_py<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    let json = py.import("json")?;
    let text = serde_json::to_string(value)
        .map_err(|error| encode_py_err(py, "encode result JSON", error))?;
    json.call_method1("loads", (text,))
}

fn outcome_py(py: Python<'_>, outcome: &JudgeOutcome) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(outcome)
        .map_err(|error| encode_py_err(py, "encode outcome JSON", error))?;
    Ok(to_py(py, &value)?.unbind())
}

fn ask_with(
    py: Python<'_>,
    store: FileSessionStore,
    logger: LoggerGuard,
    id: SessionId,
    follow_up: &str,
) -> PyResult<Py<PyAny>> {
    let follow_up = follow_up.to_owned();
    let outcome = py.detach(move || {
        let outcome = store.ask(&id, &follow_up)?;
        logger.flush();
        Ok::<_, JevError>(outcome)
    });
    outcome_py(py, &map_err(py, outcome)?)
}

/// Result of [`run_check`].
///
/// `ask` continues a Haiku or Luna session. A Jev session raises `JevError`
/// with code `INPUT_INVALID`. Dry-run leaves [`session`](Self::session) empty.
#[pyclass(module = "sc_jev_py", name = "CheckResult")]
pub struct CheckResult {
    inner: FinishedRun,
}

#[pymethods]
impl CheckResult {
    /// Session id for a live run.
    #[getter]
    fn session_id(&self) -> Option<String> {
        self.inner.session_id.map(|id| id.to_string())
    }

    /// SHA-256 of the canonical payload. This is `prompt_sha256`.
    #[getter]
    fn prompt_sha256(&self) -> &str {
        &self.inner.prompt_sha256
    }

    /// Canonical payload JSON.
    #[getter]
    fn payload(&self) -> &str {
        &self.inner.payload
    }

    /// Judge outcomes in backend order. Empty on dry-run.
    ///
    /// # Errors
    ///
    /// Raises `JevError` with code `INPUT_INVALID` when the outcomes cannot
    /// be encoded as JSON.
    #[getter]
    fn outcomes(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = serde_json::to_value(&self.inner.outcomes)
            .map_err(|error| encode_py_err(py, "encode outcomes JSON", error))?;
        Ok(to_py(py, &value)?.unbind())
    }

    /// Equal-input record. `None` on dry-run.
    ///
    /// # Errors
    ///
    /// Raises `JevError` with code `INPUT_INVALID` when the record cannot be
    /// encoded as JSON.
    #[getter]
    fn equal_input(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match &self.inner.equal_input {
            Some(record) => {
                let value = serde_json::to_value(record)
                    .map_err(|error| encode_py_err(py, "encode equal-input JSON", error))?;
                Ok(to_py(py, &value)?.unbind())
            }
            None => Ok(py.None()),
        }
    }

    /// LLM system prompt injected for this run. Not part of `prompt_sha256`.
    #[getter]
    fn system_prompt(&self) -> Option<&str> {
        self.inner.system_prompt.as_deref()
    }

    /// Session handle for [`Session::ask`]. `None` on dry-run.
    #[getter]
    fn session(&self) -> Option<Session> {
        self.inner.session_id.map(|id| Session {
            id,
            store: self.inner.store.clone(),
            logger: self.inner.logger.clone(),
        })
    }

    /// Sends `follow_up` to the live session's Haiku or Luna backend.
    ///
    /// # Errors
    ///
    /// Raises `JevError`. Dry-run uses `CONFIG_INVALID`. A Jev session uses
    /// `INPUT_INVALID`. `code` is the stable id and `message` is the fix hint.
    fn ask(&self, py: Python<'_>, follow_up: &str) -> PyResult<Py<PyAny>> {
        let Some(id) = self.inner.session_id else {
            return Err(error::to_py_err(
                py,
                &JevError::ConfigInvalid {
                    detail: "dry-run does not create a session; ask a live run".into(),
                },
            ));
        };
        ask_with(
            py,
            self.inner.store.clone(),
            self.inner.logger.clone(),
            id,
            follow_up,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "CheckResult(session_id={:?}, prompt_sha256={})",
            self.session_id(),
            self.inner.prompt_sha256
        )
    }
}

/// Live judge session returned by [`CheckResult::session`].
#[pyclass(module = "sc_jev_py", name = "Session")]
pub struct Session {
    id: SessionId,
    store: FileSessionStore,
    logger: LoggerGuard,
}

#[pymethods]
impl Session {
    /// Session id.
    #[getter]
    fn session_id(&self) -> String {
        self.id.to_string()
    }

    /// Sends `follow_up` through `sc-jev-check` and appends one JSONL line.
    ///
    /// # Errors
    ///
    /// Raises `JevError`. A Jev session uses `INPUT_INVALID` because Jev has
    /// no rationale field. `code` is the stable id and `message` is the fix hint.
    fn ask(&self, py: Python<'_>, follow_up: &str) -> PyResult<Py<PyAny>> {
        ask_with(
            py,
            self.store.clone(),
            self.logger.clone(),
            self.id,
            follow_up,
        )
    }

    fn __repr__(&self) -> String {
        format!("Session(session_id={})", self.id)
    }
}

/// Builds the canonical payload for repo-relative `check_path` without sending.
///
/// `repo_root` defaults to the process current directory. `rev` defaults to
/// `WORKTREE`. The result mapping contains `payload`, `prompt_sha256`, and
/// `producer`.
///
/// # Errors
///
/// Raises `JevError` when the check file or payload cannot be built. `code`
/// is the stable id and `message` is the fix hint.
///
/// # Examples
///
/// ```text
/// built = jev.build_request("check.toml", repo_root="/path/to/repo")
/// ```
#[pyfunction]
#[pyo3(signature = (check_path, *, repo_root=None, rev="WORKTREE"))]
pub fn build_request(
    py: Python<'_>,
    check_path: &str,
    repo_root: Option<&str>,
    rev: &str,
) -> PyResult<Py<PyAny>> {
    let built = map_err(
        py,
        build(check_path, optional_path(repo_root).as_deref(), rev),
    )?;
    let body = serde_json::json!({
        "payload": built.payload,
        "prompt_sha256": built.prompt_sha256,
        "producer": built.producer,
    });
    Ok(to_py(py, &body)?.unbind())
}

/// Runs `check_path` through `sc-jev-check`.
///
/// `backends` defaults to `["jev"]`. `dry_run` returns the payload digest and
/// does not resolve keys or send. `scripted_answers` installs an in-process
/// judge and does not open a socket; omit it to use the HTTP backends.
///
/// Log and session paths follow the crate log-root rules. See the module
/// documentation for `SC_AGENT_LOG_DIR`, `SC_LOG_ROOT`, and the JSONL path.
///
/// # Errors
///
/// Raises `JevError` with `code` and `message` set from the facade error.
///
/// # Examples
///
/// ```text
/// import sc_jev_py as jev
/// result = jev.run_check("check.toml", backends=["jev"], dry_run=False)
/// ```
#[pyfunction]
#[pyo3(signature = (
    check_path,
    *,
    backends=None,
    dry_run=false,
    repo_root=None,
    rev="WORKTREE",
    log_dir=None,
    session_dir=None,
    system_prompt_file=None,
    scripted_answers=None,
))]
#[allow(clippy::too_many_arguments)]
pub fn run_check(
    py: Python<'_>,
    check_path: &str,
    backends: Option<Vec<String>>,
    dry_run: bool,
    repo_root: Option<&str>,
    rev: &str,
    log_dir: Option<&str>,
    session_dir: Option<&str>,
    system_prompt_file: Option<&str>,
    scripted_answers: Option<Bound<'_, PyAny>>,
) -> PyResult<CheckResult> {
    let scripted_answers = match scripted_answers {
        Some(answers) => Some(map_err(py, py_to_value(&answers))?),
        None => None,
    };
    let spec = CheckRun {
        check_path: check_path.to_owned(),
        backends: backends.unwrap_or_else(|| vec!["jev".to_owned()]),
        dry_run,
        repo_root: optional_path(repo_root),
        rev: rev.to_owned(),
        log_dir: optional_path(log_dir),
        session_dir: optional_path(session_dir),
        system_prompt_file: optional_path(system_prompt_file),
        scripted_answers,
    };
    let finished = py.detach(move || execute(spec));
    Ok(CheckResult {
        inner: map_err(py, finished)?,
    })
}

/// Returns canonical payload bytes for `json_text`.
///
/// Python must pass the JSON text through. This function parses it in Rust
/// and encodes ADR-SAG-0002 bytes. It does not accept a pre-canonicalized
/// digest from Python.
///
/// # Errors
///
/// Raises `JevError` with code `INPUT_INVALID` when `json_text` is not JSON.
#[pyfunction]
pub fn canonical_bytes(py: Python<'_>, json_text: &str) -> PyResult<Py<PyBytes>> {
    let bytes = map_err(py, canonical_of(json_text))?;
    Ok(PyBytes::new(py, &bytes).unbind())
}

/// Returns the SHA-256 hex digest of [`canonical_bytes`].
///
/// # Errors
///
/// Raises `JevError` with code `INPUT_INVALID` when `json_text` is not JSON.
#[pyfunction]
pub fn canonical_sha256(py: Python<'_>, json_text: &str) -> PyResult<String> {
    let bytes = map_err(py, canonical_of(json_text))?;
    Ok(sc_agent_types::Sha256Digest::of(&bytes).to_string())
}

/// Returns the default log root (`~/.sc-agent/log`).
#[pyfunction]
#[must_use]
pub fn default_log_root() -> String {
    logging::default_log_root().display().to_string()
}

/// Resolves the log root from `log_dir`, then the environment, then the default.
///
/// See the module documentation for `SC_AGENT_LOG_DIR` and `SC_LOG_ROOT`.
#[pyfunction]
#[pyo3(signature = (log_dir=None))]
#[must_use]
pub fn resolve_log_root(log_dir: Option<&str>) -> String {
    logging::resolve_log_root(optional_path(log_dir).as_deref())
        .display()
        .to_string()
}

/// Resolves the session directory for `log_dir`.
///
/// `session_dir` wins, then `SC_AGENT_SESSION_DIR`, then `{log_dir}/sessions`.
#[pyfunction]
#[pyo3(signature = (log_dir, *, session_dir=None))]
#[must_use]
pub fn resolve_session_dir(log_dir: &str, session_dir: Option<&str>) -> String {
    logging::resolve_session_dir(optional_path(session_dir).as_deref(), Path::new(log_dir))
        .display()
        .to_string()
}

/// Returns `{log_dir}/logs/sc-jev-py.log.jsonl`.
#[pyfunction]
#[must_use]
pub fn log_file(log_dir: &str) -> String {
    logging::log_file(Path::new(log_dir)).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_jev_check::session::SessionStore;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_dir(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("sc-jev-py-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }

    fn write_questions(dir: &Path, instructions: &str) {
        let questions = format!(
            r#"{{
              "schema": "plan-anchoring/questions/1",
              "version": 1,
              "questions": {{
                "relevant": {{
                  "type": "choice",
                  "instructions": {instructions:?},
                  "criteria": {{"no": "No.", "yes": "Yes."}}
                }}
              }}
            }}"#
        );
        std::fs::write(dir.join("questions.json"), questions).expect("questions");
        std::fs::write(
            dir.join("check.toml"),
            "[questions]\npath = \"questions.json\"\n",
        )
        .expect("check");
    }

    fn run_spec(
        repo: &Path,
        logs: &Path,
        sessions: &Path,
        backend: &str,
        dry_run: bool,
    ) -> CheckRun {
        CheckRun {
            check_path: "check.toml".to_owned(),
            backends: vec![backend.to_owned()],
            dry_run,
            repo_root: Some(repo.to_path_buf()),
            rev: "WORKTREE".to_owned(),
            log_dir: Some(logs.to_path_buf()),
            session_dir: Some(sessions.to_path_buf()),
            system_prompt_file: None,
            scripted_answers: Some(serde_json::json!({"relevant": "yes"})),
        }
    }

    fn jsonl_lines(logs: &Path) -> Vec<String> {
        let path = logging::log_file(logs);
        if !path.exists() {
            return Vec::new();
        }
        std::fs::read_to_string(path)
            .expect("jsonl")
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn canonical_bytes_match_a1_fixture() {
        let folder = workspace_root().join("tests/fixtures/jev/cross_impl");
        let text = std::fs::read_to_string(folder.join("logical.json")).expect("logical");
        let bytes = canonical_of(&text).expect("canonical");
        let reference = std::fs::read(folder.join("reference.bytes")).expect("reference bytes");
        let digest = std::fs::read_to_string(folder.join("reference.sha256")).expect("digest");
        assert_eq!(bytes, reference);
        assert_eq!(
            sc_agent_types::Sha256Digest::of(&bytes).as_str(),
            digest.trim()
        );
    }

    #[test]
    fn branches_fixture_payload_includes_subjects() {
        let root = temp_dir("branches");
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        write_questions(&repo, "Branches:\n\n{branches}");
        let subjects = workspace_root().join("tests/fixtures/cross-impl/branches/subjects.json");
        std::fs::copy(subjects, repo.join("subjects.json")).expect("copy subjects");
        std::fs::write(
            repo.join("check.toml"),
            "[questions]\npath = \"questions.json\"\n\n[inputs.branches]\nkind = \"subjects\"\npath = \"subjects.json\"\n",
        )
        .expect("check");
        let built = build("check.toml", Some(&repo), "WORKTREE").expect("build");
        assert!(built.payload.contains("Transition.GIVE_BACK"));
        assert!(
            built.payload.contains("Refusal(\\\"give-back\\\")")
                || built.payload.contains("give-back")
        );
        let producer = built.producer.expect("producer");
        assert_eq!(producer.name, "sc-treesitter");
        assert_eq!(producer.language, "python");
        assert_eq!(producer.mode, "matches");
        let again = build("check.toml", Some(&repo), "WORKTREE").expect("rebuild");
        assert_eq!(built.prompt_sha256, again.prompt_sha256);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn git_repo_dry_run_has_no_session_and_no_jsonl() {
        let root = temp_dir("dry");
        let logs = root.join("log");
        let sessions = root.join("sessions");
        let repo = workspace_root().join("tests/fixtures/jev/git-repo");
        let finished = execute(run_spec(&repo, &logs, &sessions, "jev", true)).expect("dry-run");
        assert!(finished.session_id.is_none());
        assert!(finished.payload.contains("alpha note"));
        assert!(finished.outcomes.is_empty());
        assert!(finished.equal_input.is_none());
        assert!(jsonl_lines(&logs).is_empty());
        drop(finished);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn live_run_appends_one_jsonl_line_per_send_without_the_prompt() {
        let root = temp_dir("live");
        let repo = root.join("repo");
        let logs = root.join("log");
        let sessions = root.join("sessions");
        std::fs::create_dir_all(&repo).expect("repo");
        let sentinel = "sentinel-question-text-not-for-logs";
        write_questions(&repo, sentinel);
        let finished =
            execute(run_spec(&repo, &logs, &sessions, "anthropic_haiku", false)).expect("run");
        assert!(finished.session_id.is_some());
        assert!(finished.payload.contains(sentinel));
        assert!(!finished.system_prompt.as_deref().unwrap_or("").is_empty());
        let lines = jsonl_lines(&logs);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("judge"));
        assert!(lines[0].contains(&finished.prompt_sha256));
        assert!(lines[0].contains("anthropic_haiku"));
        assert!(!lines[0].contains(sentinel));
        assert!(!lines[0].to_ascii_lowercase().contains("api_key"));
        let follow = finished
            .store
            .ask(&finished.session_id.expect("session"), "why?");
        finished.logger.flush();
        let outcome = follow.expect("ask");
        assert_eq!(outcome.backend, BackendId::AnthropicHaiku);
        let lines = jsonl_lines(&logs);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[1].contains("ask"));
        assert!(!lines[1].contains(sentinel));
        assert!(!lines[1].contains("why?"));
        drop(finished);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn jev_ask_is_input_invalid_and_missing_check_is_input_missing() {
        let root = temp_dir("errors");
        let repo = root.join("repo");
        let logs = root.join("log");
        let sessions = root.join("sessions");
        std::fs::create_dir_all(&repo).expect("repo");
        write_questions(&repo, "Does it apply?");
        let finished = execute(run_spec(&repo, &logs, &sessions, "jev", false)).expect("run");
        let error = finished
            .store
            .ask(&finished.session_id.expect("session"), "why?")
            .expect_err("jev ask");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.fix_hint().contains("rationale"));
        assert_eq!(jsonl_lines(&logs).len(), 1);
        let missing = execute(CheckRun {
            check_path: "missing.toml".to_owned(),
            backends: vec!["jev".to_owned()],
            dry_run: false,
            repo_root: Some(repo.clone()),
            rev: "WORKTREE".to_owned(),
            log_dir: Some(logs.clone()),
            session_dir: Some(sessions),
            system_prompt_file: None,
            scripted_answers: None,
        });
        let Err(missing) = missing else {
            panic!("missing check should fail");
        };
        assert_eq!(missing.code(), "INPUT_MISSING");
        assert!(!missing.fix_hint().to_ascii_lowercase().contains("api_key"));
        drop(finished);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn serde_encode_failure_is_jev_error() {
        struct FailWriter;
        impl std::io::Write for FailWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("encode refused"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let failure = serde_json::to_writer(FailWriter, &serde_json::json!({"ok": true}))
            .expect_err("writer");
        let error = encode_failure("encode outcomes JSON", &failure);
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.fix_hint().contains("encode outcomes JSON"));
        Python::initialize();
        Python::attach(|py| {
            let py_err = error::to_py_err(py, &error);
            assert!(py_err.is_instance_of::<crate::error::JevError>(py));
            assert!(!py_err.is_instance_of::<pyo3::exceptions::PyValueError>(py));
            let code: String = py_err
                .value(py)
                .getattr("code")
                .expect("code")
                .extract()
                .expect("code str");
            assert_eq!(code, "INPUT_INVALID");
        });
    }

    #[test]
    fn scripted_answers_that_are_not_json_are_input_invalid() {
        Python::initialize();
        Python::attach(|py| {
            let nan = py.eval(c"float('nan')", None, None).expect("nan");
            let error = py_to_value(&nan).expect_err("nan is not JSON");
            assert_eq!(error.code(), "INPUT_INVALID");
            assert!(error.fix_hint().contains("scripted_answers"));

            let opaque = py.eval(c"object()", None, None).expect("object");
            let error = py_to_value(&opaque).expect_err("object is not JSON");
            assert_eq!(error.code(), "INPUT_INVALID");
            assert!(error.fix_hint().contains("scripted_answers"));
        });
    }
}
