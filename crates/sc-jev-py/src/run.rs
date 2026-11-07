//! `build_request`, `run_check`, and session `ask`.
//!
//! Every pyfunction here is `pub(crate)`: the Python module registered in
//! `lib.rs` is the only surface. Work that may block (git, extractors, judge
//! sends) runs under `py.detach`, so other Python threads keep running.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBytes};
use sc_agent_types::schema::subjects::SubjectsProducer;
use sc_agent_types::{
    BackendId, EqualInputRecord, JevError, JudgeOutcome, RepoRoot, Rev, SessionId, Sha256Digest,
    canonical_json_bytes,
};
use sc_jev_check::config::load_check_at;
use sc_jev_check::run::{
    RunOptions, build_request as build_request_facade, run_check as run_check_facade,
};
use sc_jev_check::session::{FileSessionStore, SessionStore};
use serde_json::Value;

use crate::error::{self, map_err};
use crate::logging::{self, LoggerGuard};

/// Default `backends` when the caller passes none.
const DEFAULT_BACKEND: &str = "jev";

/// One Python `run_check` invocation, with owned arguments.
struct CheckRun {
    check_path: String,
    backends: Vec<String>,
    dry_run: bool,
    repo_root: Option<PathBuf>,
    rev: String,
    log_dir: Option<PathBuf>,
    session_dir: Option<PathBuf>,
    system_prompt_file: Option<PathBuf>,
    /// Scripted judge answers; only `cargo test` and the `test-util` feature
    /// can set this (ADR-SAG-0022).
    #[cfg(any(test, feature = "test-util"))]
    scripted_answers: Option<Value>,
}

/// Session store and logger kept alive so `ask` reuses them.
struct LiveSession {
    store: FileSessionStore,
    logger: LoggerGuard,
}

/// Facade result for one run. `live` is `None` on dry-run.
struct FinishedRun {
    session_id: Option<SessionId>,
    payload: String,
    prompt_sha256: Sha256Digest,
    outcomes: Vec<JudgeOutcome>,
    equal_input: Option<EqualInputRecord>,
    system_prompt: Option<String>,
    live: Option<LiveSession>,
}

struct PreparedRequest {
    payload: String,
    prompt_sha256: Sha256Digest,
    producer: Option<SubjectsProducer>,
}

/// Canonical JSON bytes for `json_text` (ADR-SAG-0002).
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when `json_text` is not JSON or cannot
/// be encoded.
pub(crate) fn canonical_of(json_text: &str) -> Result<Vec<u8>, JevError> {
    let value: Value = serde_json::from_str(json_text)
        .map_err(|error| JevError::encode_failed("JSON text", &error))?;
    canonical_json_bytes(&value)
}

fn execute(spec: CheckRun) -> Result<FinishedRun, JevError> {
    let backends = parse_backends(&spec.backends)?;
    let repo = resolve_repo(spec.repo_root.as_deref())?;
    let rev = Rev::parse(&spec.rev)?;
    let check = load_check_at(&repo, &rev, &spec.check_path)?;
    let (mut options, live) = if spec.dry_run {
        // Dry-run creates no session store, session directory, or logger.
        (RunOptions::dry_run_only(repo).rev(rev), None)
    } else {
        let log_root = logging::resolve_log_root(spec.log_dir.as_deref());
        let session_dir = logging::resolve_session_dir(spec.session_dir.as_deref(), &log_root);
        let (logger, session_log) = logging::open_session_log(&log_root)?;
        let store = FileSessionStore::new(&session_dir, Arc::clone(&session_log))?;
        let options = RunOptions::new(repo, store.clone(), session_log).rev(rev);
        (options, Some(LiveSession { store, logger }))
    };
    if let Some(path) = spec.system_prompt_file {
        options = options.system_prompt_override(path);
    }
    #[cfg(any(test, feature = "test-util"))]
    if let Some(answers) = spec.scripted_answers {
        options = options.registry(sc_jev_check::registry::register_scripted_backends(
            &backends, answers,
        )?);
    }
    let result = run_check_facade(&check, &backends, options)?;
    if let Some(live) = &live {
        live.logger.flush();
    }
    let (payload, prompt_sha256) = result.bundle.into_text();
    Ok(FinishedRun {
        session_id: result.session_id,
        prompt_sha256,
        payload,
        outcomes: result.outcomes,
        equal_input: result.equal_input,
        system_prompt: result.system_prompt,
        live,
    })
}

fn build(
    check_path: &str,
    repo_root: Option<&Path>,
    rev_text: &str,
) -> Result<PreparedRequest, JevError> {
    let repo = resolve_repo(repo_root)?;
    let rev = Rev::parse(rev_text)?;
    let built = build_request_facade(&repo, &rev, check_path)?;
    let (payload, prompt_sha256) = built.bundle.into_text();
    Ok(PreparedRequest {
        payload,
        prompt_sha256,
        producer: built.producer,
    })
}

/// Parses backend ids with [`BackendId`]'s `FromStr`.
///
/// Only `jev`, `anthropic_haiku`, and `openai_luna` are accepted, matching
/// `sc-ai` and the engine. Kebab-case aliases (`anthropic-haiku`) are not
/// accepted. An unknown id is `INPUT_INVALID`. An empty list is left to the
/// engine, which reports `CONFIG_INVALID`.
fn parse_backends(names: &[String]) -> Result<Vec<BackendId>, JevError> {
    names.iter().map(|name| name.parse()).collect()
}

/// Resolves `repo_root` like `sc-ai --repo-root`.
///
/// An explicit root must be a directory and is made absolute. `None` (or an
/// empty path) is the process current directory.
fn resolve_repo(repo_root: Option<&Path>) -> Result<RepoRoot, JevError> {
    match repo_root.filter(|path| !path.as_os_str().is_empty()) {
        Some(path) => {
            if !path.is_dir() {
                return Err(JevError::input_missing(path.display().to_string()));
            }
            let absolute = std::path::absolute(path)
                .map_err(|error| JevError::io_failed("repo_root", &error))?;
            Ok(RepoRoot::new(absolute))
        }
        None => {
            let current = std::env::current_dir()
                .map_err(|error| JevError::io_failed("current directory", &error))?;
            Ok(RepoRoot::new(current))
        }
    }
}

/// Drops an empty path, which Python callers use to mean "unset".
fn non_empty(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| !path.as_os_str().is_empty())
}

/// Repo-relative check path as text.
fn check_path_text(check_path: &Path) -> Result<String, JevError> {
    check_path.to_str().map(str::to_owned).ok_or_else(|| {
        JevError::input_invalid(format!(
            "check path {} is not valid UTF-8",
            check_path.display()
        ))
    })
}

#[cfg(any(test, feature = "test-util"))]
fn py_to_value(obj: &Bound<'_, PyAny>) -> Result<Value, JevError> {
    let text = python_json(obj).map_err(|error| {
        JevError::input_invalid(format!("scripted_answers is not JSON: {error}"))
    })?;
    serde_json::from_str(&text).map_err(|error| JevError::encode_failed("scripted_answers", &error))
}

#[cfg(any(test, feature = "test-util"))]
fn python_json(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    let json = obj.py().import("json")?;
    json.call_method1("dumps", (obj,))?.extract()
}

fn encode_py_err(py: Python<'_>, label: &str, error: &serde_json::Error) -> PyErr {
    error::to_py_err(py, &JevError::encode_failed(label, error))
}

/// Converts `value` (from `serde_json::to_value`) to a Python object via JSON.
fn to_py(py: Python<'_>, label: &str, value: serde_json::Result<Value>) -> PyResult<Py<PyAny>> {
    let text = value
        .and_then(|value| serde_json::to_string(&value))
        .map_err(|error| encode_py_err(py, label, &error))?;
    Ok(py.import("json")?.call_method1("loads", (text,))?.unbind())
}

fn ask_with(
    py: Python<'_>,
    live: &LiveSession,
    id: SessionId,
    follow_up: &str,
) -> PyResult<Py<PyAny>> {
    let follow_up = follow_up.to_owned();
    let store = live.store.clone();
    let logger = live.logger.clone();
    let outcome = py.detach(move || {
        let outcome = store.ask(&id, &follow_up)?;
        logger.flush();
        Ok::<_, JevError>(outcome)
    });
    to_py(
        py,
        "outcome JSON",
        serde_json::to_value(map_err(py, outcome)?),
    )
}

/// Result of `run_check`.
///
/// Immutable. `outcomes` and `equal_input` are converted to Python objects
/// once, when the result is built. `ask` is kept beside `Session.ask`
/// because the Python harness relies on it; both send through the same
/// session. Dry-run leaves `session` empty.
#[pyclass(module = "sc_jev_py", name = "CheckResult", frozen)]
pub(crate) struct CheckResult {
    inner: FinishedRun,
    outcomes: Py<PyAny>,
    equal_input: Py<PyAny>,
}

impl CheckResult {
    fn new(py: Python<'_>, inner: FinishedRun) -> PyResult<Self> {
        let outcomes = to_py(py, "outcomes JSON", serde_json::to_value(&inner.outcomes))?;
        let equal_input = match &inner.equal_input {
            Some(record) => to_py(py, "equal-input JSON", serde_json::to_value(record))?,
            None => py.None(),
        };
        Ok(Self {
            inner,
            outcomes,
            equal_input,
        })
    }
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
        self.inner.prompt_sha256.as_str()
    }

    /// Canonical payload JSON.
    #[getter]
    fn payload(&self) -> &str {
        &self.inner.payload
    }

    /// Judge outcomes in backend order. Empty on dry-run.
    #[getter]
    fn outcomes(&self, py: Python<'_>) -> Py<PyAny> {
        self.outcomes.clone_ref(py)
    }

    /// Equal-input record. `None` on dry-run.
    #[getter]
    fn equal_input(&self, py: Python<'_>) -> Py<PyAny> {
        self.equal_input.clone_ref(py)
    }

    /// LLM system prompt injected for this run. Not part of `prompt_sha256`.
    #[getter]
    fn system_prompt(&self) -> Option<&str> {
        self.inner.system_prompt.as_deref()
    }

    /// Session handle for `Session.ask`. `None` on dry-run.
    #[getter]
    fn session(&self) -> Option<Session> {
        let live = self.inner.live.as_ref()?;
        self.inner.session_id.map(|id| Session {
            id,
            live: LiveSession {
                store: live.store.clone(),
                logger: live.logger.clone(),
            },
        })
    }

    /// Sends `follow_up` to the live session's Haiku or Luna backend.
    ///
    /// Same as `self.session.ask(follow_up)`.
    ///
    /// # Errors
    ///
    /// Raises `JevError`. Dry-run uses `CONFIG_INVALID`. A Jev session uses
    /// `INPUT_INVALID`. `code` is the stable id and `message` is the fix hint.
    fn ask(&self, py: Python<'_>, follow_up: &str) -> PyResult<Py<PyAny>> {
        let (Some(id), Some(live)) = (self.inner.session_id, self.inner.live.as_ref()) else {
            return Err(error::to_py_err(
                py,
                &JevError::config_invalid("dry-run does not create a session; ask a live run"),
            ));
        };
        ask_with(py, live, id, follow_up)
    }

    fn __repr__(&self) -> String {
        format!(
            "CheckResult(session_id={:?}, prompt_sha256={})",
            self.session_id(),
            self.inner.prompt_sha256
        )
    }
}

/// Live judge session returned by `CheckResult.session`.
#[pyclass(module = "sc_jev_py", name = "Session", frozen)]
pub(crate) struct Session {
    id: SessionId,
    live: LiveSession,
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
        ask_with(py, &self.live, self.id, follow_up)
    }

    fn __repr__(&self) -> String {
        format!("Session(session_id={})", self.id)
    }
}

/// Builds the canonical payload for repo-relative `check_path` without sending.
///
/// Assembly is `sc_jev_check::run::build_request` (ADR-SAG-0005). This
/// function maps that result into Python. It does not run extractors or call
/// `PayloadBuilder`. `repo_root` defaults to the process current directory.
/// `rev` defaults to `WORKTREE`. The GIL is released while the request is
/// assembled (git reads and command extractors may take seconds). The result
/// mapping contains `payload`, `prompt_sha256`, and `producer`.
///
/// # Errors
///
/// Raises `JevError` when the check file or payload cannot be built. `code`
/// is the stable id and `message` is the fix hint.
///
/// # Examples
///
/// ```text
/// built = jev.build_request("check.toml", repo_root=pathlib.Path("/path/to/repo"))
/// ```
#[pyfunction]
#[pyo3(signature = (check_path, *, repo_root=None, rev="WORKTREE"))]
pub(crate) fn build_request(
    py: Python<'_>,
    check_path: PathBuf,
    repo_root: Option<PathBuf>,
    rev: &str,
) -> PyResult<Py<PyAny>> {
    let check_path = map_err(py, check_path_text(&check_path))?;
    let repo_root = non_empty(repo_root);
    let rev = rev.to_owned();
    let built = py.detach(move || build(&check_path, repo_root.as_deref(), &rev));
    let built = map_err(py, built)?;
    let body = serde_json::json!({
        "payload": built.payload,
        "prompt_sha256": built.prompt_sha256,
        "producer": built.producer,
    });
    to_py(py, "build_request JSON", Ok(body))
}

/// Builds the owned [`CheckRun`] shared by both Python entry points.
#[expect(clippy::too_many_arguments, reason = "mirrors the Python keyword API")]
fn check_run(
    check_path: &Path,
    backends: Option<Vec<String>>,
    dry_run: bool,
    repo_root: Option<PathBuf>,
    rev: String,
    log_dir: Option<PathBuf>,
    session_dir: Option<PathBuf>,
    system_prompt_file: Option<PathBuf>,
) -> Result<CheckRun, JevError> {
    Ok(CheckRun {
        check_path: check_path_text(check_path)?,
        backends: backends.unwrap_or_else(|| vec![DEFAULT_BACKEND.to_owned()]),
        dry_run,
        repo_root: non_empty(repo_root),
        rev,
        log_dir: non_empty(log_dir),
        session_dir: non_empty(session_dir),
        system_prompt_file: non_empty(system_prompt_file),
        #[cfg(any(test, feature = "test-util"))]
        scripted_answers: None,
    })
}

fn finish_check(py: Python<'_>, spec: CheckRun) -> PyResult<CheckResult> {
    let finished = py.detach(move || execute(spec));
    CheckResult::new(py, map_err(py, finished)?)
}

/// Runs `check_path` through `sc-jev-check`.
///
/// `backends` defaults to `["jev"]`; ids are `jev`, `anthropic_haiku`, and
/// `openai_luna`. `dry_run` returns the payload digest and does not resolve
/// keys, send, or create a session store or log. Path arguments accept `str`
/// or `os.PathLike`. This function does not register a judge. Live runs use
/// the HTTP backends inside `sc-jev-check` (ADR-SAG-0009). The GIL is
/// released while the check runs.
///
/// Log and session paths follow ADR-SAG-0021. See the module documentation
/// for `SC_AGENT_LOG_DIR`, `SC_LOG_ROOT`, and the JSONL path.
///
/// # Errors
///
/// Raises `JevError` with `code` and `message` set from the facade error. An
/// unknown backend id or a malformed `rev` is `INPUT_INVALID`.
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
))]
#[expect(clippy::too_many_arguments, reason = "Python keyword API")]
pub(crate) fn run_check(
    py: Python<'_>,
    check_path: PathBuf,
    backends: Option<Vec<String>>,
    dry_run: bool,
    repo_root: Option<PathBuf>,
    rev: &str,
    log_dir: Option<PathBuf>,
    session_dir: Option<PathBuf>,
    system_prompt_file: Option<PathBuf>,
) -> PyResult<CheckResult> {
    let spec = map_err(
        py,
        check_run(
            &check_path,
            backends,
            dry_run,
            repo_root,
            rev.to_owned(),
            log_dir,
            session_dir,
            system_prompt_file,
        ),
    )?;
    finish_check(py, spec)
}

/// Test-only entry points, registered as `sc_jev_py._testing` (ADR-SAG-0022).
#[cfg(any(test, feature = "test-util"))]
pub(crate) mod testing {
    use super::*;

    /// `run_check` with scripted judge answers instead of HTTP backends.
    ///
    /// `scripted_answers` is a mapping of question id to an answer object or
    /// the string shorthand for `{"choice": ...}`. It is forwarded to
    /// `sc_jev_check::registry::register_scripted_backends`, which owns the
    /// shape rules. This crate implements no judge (ADR-SAG-0009).
    ///
    /// # Errors
    ///
    /// Raises `JevError` like `run_check`. `scripted_answers` that is not JSON
    /// or not a mapping of question ids is `INPUT_INVALID`.
    #[pyfunction]
    #[pyo3(signature = (
        check_path,
        *,
        scripted_answers,
        backends=None,
        dry_run=false,
        repo_root=None,
        rev="WORKTREE",
        log_dir=None,
        session_dir=None,
        system_prompt_file=None,
    ))]
    #[expect(clippy::too_many_arguments, reason = "Python keyword API")]
    pub(crate) fn run_check_scripted(
        py: Python<'_>,
        check_path: PathBuf,
        scripted_answers: Bound<'_, PyAny>,
        backends: Option<Vec<String>>,
        dry_run: bool,
        repo_root: Option<PathBuf>,
        rev: &str,
        log_dir: Option<PathBuf>,
        session_dir: Option<PathBuf>,
        system_prompt_file: Option<PathBuf>,
    ) -> PyResult<CheckResult> {
        let answers = map_err(py, py_to_value(&scripted_answers))?;
        let mut spec = map_err(
            py,
            check_run(
                &check_path,
                backends,
                dry_run,
                repo_root,
                rev.to_owned(),
                log_dir,
                session_dir,
                system_prompt_file,
            ),
        )?;
        spec.scripted_answers = Some(answers);
        finish_check(py, spec)
    }
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
pub(crate) fn canonical_bytes(py: Python<'_>, json_text: &str) -> PyResult<Py<PyBytes>> {
    let bytes = map_err(py, canonical_of(json_text))?;
    Ok(PyBytes::new(py, &bytes).unbind())
}

/// Returns the SHA-256 hex digest of `canonical_bytes(json_text)`.
///
/// # Errors
///
/// Raises `JevError` with code `INPUT_INVALID` when `json_text` is not JSON.
#[pyfunction]
pub(crate) fn canonical_sha256(py: Python<'_>, json_text: &str) -> PyResult<String> {
    let bytes = map_err(py, canonical_of(json_text))?;
    Ok(Sha256Digest::of(&bytes).to_string())
}

/// Returns the default log root (`~/.sc-agent/log`).
#[pyfunction]
#[must_use]
pub(crate) fn default_log_root() -> PathBuf {
    logging::default_log_root()
}

/// Resolves the log root (ADR-SAG-0021).
///
/// First match wins: `log_dir`, then `SC_AGENT_LOG_DIR`, then `SC_LOG_ROOT`,
/// then `~/.sc-agent/log` (`%USERPROFILE%\.sc-agent\log` when `HOME` is unset).
/// When both home variables are unset, the path is the process temp directory
/// plus `sc-agent/log`. An empty argument or environment value is absent.
#[pyfunction]
#[pyo3(signature = (log_dir=None))]
#[must_use]
pub(crate) fn resolve_log_root(log_dir: Option<PathBuf>) -> PathBuf {
    logging::resolve_log_root(log_dir.as_deref())
}

/// Resolves the session directory (ADR-SAG-0021).
///
/// `log_dir` is the log root from `resolve_log_root`. First match wins:
/// `session_dir`, then `SC_AGENT_SESSION_DIR`, then `{log_dir}/sessions`.
/// An empty `session_dir` or environment value is absent.
#[pyfunction]
#[pyo3(signature = (log_dir, *, session_dir=None))]
#[must_use]
pub(crate) fn resolve_session_dir(log_dir: PathBuf, session_dir: Option<PathBuf>) -> PathBuf {
    logging::resolve_session_dir(session_dir.as_deref(), &log_dir)
}

/// Returns `{log_dir}/logs/sc-jev-py.log.jsonl`.
#[pyfunction]
#[must_use]
pub(crate) fn log_file(log_dir: PathBuf) -> PathBuf {
    logging::log_file(&log_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_jev_check::session::SessionStore;
    use tempfile::TempDir;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_dir(label: &str) -> TempDir {
        tempfile::Builder::new()
            .prefix(&format!("sc-jev-py-{label}-"))
            .tempdir()
            .expect("temp dir")
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

    fn live(finished: &FinishedRun) -> &LiveSession {
        finished.live.as_ref().expect("live run")
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
    fn branches_fixture_payload_includes_subjects() {
        let root = temp_dir("branches");
        let repo = root.path();
        write_questions(repo, "Branches:\n\n{branches}");
        let subjects = workspace_root().join("tests/fixtures/cross-impl/branches/subjects.json");
        std::fs::copy(subjects, repo.join("subjects.json")).expect("copy subjects");
        std::fs::write(
            repo.join("check.toml"),
            "[questions]\npath = \"questions.json\"\n\n[inputs.branches]\nkind = \"subjects\"\npath = \"subjects.json\"\n",
        )
        .expect("check");
        let built = build("check.toml", Some(repo), "WORKTREE").expect("build");
        assert!(built.payload.contains("Transition.GIVE_BACK"));
        let producer = built.producer.expect("producer");
        assert_eq!(producer.name, "sc-treesitter");
        assert_eq!(producer.language, "python");
        assert_eq!(producer.mode, "matches");
        let again = build("check.toml", Some(repo), "WORKTREE").expect("rebuild");
        assert_eq!(built.prompt_sha256, again.prompt_sha256);
    }

    #[test]
    fn git_repo_dry_run_creates_no_session_store_or_log() {
        let root = temp_dir("dry");
        let logs = root.path().join("log");
        let sessions = root.path().join("sessions");
        let repo = workspace_root().join("tests/fixtures/jev/git-repo");
        let finished = execute(run_spec(&repo, &logs, &sessions, "jev", true)).expect("dry-run");
        assert!(finished.session_id.is_none());
        assert!(finished.live.is_none());
        assert!(finished.payload.contains("alpha note"));
        assert!(finished.outcomes.is_empty());
        assert!(finished.equal_input.is_none());
        assert!(!logs.exists(), "dry-run must not create the log root");
        assert!(
            !sessions.exists(),
            "dry-run must not create the session dir"
        );
    }

    #[test]
    fn live_run_appends_one_jsonl_line_per_send_without_the_prompt() {
        let root = temp_dir("live");
        let repo = root.path().join("repo");
        let logs = root.path().join("log");
        let sessions = root.path().join("sessions");
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
        assert!(lines[0].contains(finished.prompt_sha256.as_str()));
        assert!(lines[0].contains("anthropic_haiku"));
        assert!(!lines[0].contains(sentinel));
        assert!(!lines[0].to_ascii_lowercase().contains("api_key"));
        let follow = live(&finished)
            .store
            .ask(&finished.session_id.expect("session"), "why?");
        live(&finished).logger.flush();
        let outcome = follow.expect("ask");
        assert_eq!(outcome.backend(), BackendId::AnthropicHaiku);
        let lines = jsonl_lines(&logs);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[1].contains("ask"));
        assert!(!lines[1].contains(sentinel));
        assert!(!lines[1].contains("why?"));
    }

    #[test]
    fn two_runs_in_one_process_share_one_writer() {
        let root = temp_dir("shared");
        let repo = root.path().join("repo");
        let logs = root.path().join("log");
        let sessions = root.path().join("sessions");
        std::fs::create_dir_all(&repo).expect("repo");
        write_questions(&repo, "Shared?");
        let first = execute(run_spec(&repo, &logs, &sessions, "jev", false)).expect("first");
        let second = execute(run_spec(&repo, &logs, &sessions, "jev", false)).expect("second");
        let first_logger = &live(&first).logger;
        let second_logger = &live(&second).logger;
        assert!(
            first_logger.shares_writer_with(second_logger),
            "one writer per log root"
        );
        assert_eq!(jsonl_lines(&logs).len(), 2);
    }

    #[test]
    fn jev_ask_is_input_invalid_and_missing_check_is_input_missing() {
        let root = temp_dir("errors");
        let repo = root.path().join("repo");
        let logs = root.path().join("log");
        let sessions = root.path().join("sessions");
        std::fs::create_dir_all(&repo).expect("repo");
        write_questions(&repo, "Does it apply?");
        let finished = execute(run_spec(&repo, &logs, &sessions, "jev", false)).expect("run");
        let error = live(&finished)
            .store
            .ask(&finished.session_id.expect("session"), "why?")
            .expect_err("jev ask");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.fix_hint().contains("rationale"));
        assert_eq!(jsonl_lines(&logs).len(), 1);
        let mut spec = run_spec(&repo, &logs, &sessions, "jev", false);
        spec.check_path = "missing.toml".to_owned();
        spec.scripted_answers = None;
        let Err(missing) = execute(spec) else {
            panic!("missing check should fail");
        };
        assert_eq!(missing.code(), "INPUT_MISSING");
        assert!(!missing.fix_hint().to_ascii_lowercase().contains("api_key"));
    }

    #[test]
    fn backend_ids_use_from_str_and_reject_kebab_aliases() {
        assert_eq!(
            parse_backends(&["jev".into(), "anthropic_haiku".into(), "openai_luna".into()])
                .expect("documented ids"),
            [
                BackendId::Jev,
                BackendId::AnthropicHaiku,
                BackendId::OpenAiLuna
            ]
        );
        for alias in ["anthropic-haiku", "openai-luna", "not-a-backend"] {
            let error = parse_backends(&[alias.to_owned()]).expect_err(alias);
            assert_eq!(error.code(), "INPUT_INVALID", "{alias}");
            assert!(error.to_string().contains(alias), "{error}");
        }
    }

    #[test]
    fn malformed_rev_and_missing_repo_are_typed() {
        let root = temp_dir("rev");
        write_questions(root.path(), "Rev?");
        for rev in ["", "--output=x", "a b"] {
            let error = build("check.toml", Some(root.path()), rev)
                .err()
                .expect("malformed rev");
            assert_eq!(error.code(), "INPUT_INVALID", "{rev:?}");
        }
        let absent = root.path().join("absent");
        let error = resolve_repo(Some(&absent)).expect_err("absent repo");
        assert_eq!(error.code(), "INPUT_MISSING");
        let file = root.path().join("check.toml");
        assert_eq!(
            resolve_repo(Some(&file)).expect_err("file repo").code(),
            "INPUT_MISSING"
        );
        let relative = resolve_repo(Some(Path::new("."))).expect("relative repo");
        assert!(relative.as_path().is_absolute());
        let empty = resolve_repo(Some(Path::new(""))).expect("empty means cwd");
        assert_eq!(
            empty.as_path(),
            std::env::current_dir().expect("cwd").as_path()
        );
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
        Python::initialize();
        Python::attach(|py| {
            let py_err = encode_py_err(py, "outcomes JSON", &failure);
            assert!(py_err.is_instance_of::<crate::error::JevError>(py));
            assert!(!py_err.is_instance_of::<pyo3::exceptions::PyValueError>(py));
            let code: String = py_err
                .value(py)
                .getattr("code")
                .expect("code")
                .extract()
                .expect("code str");
            assert_eq!(code, "INPUT_INVALID");
            let message: String = py_err
                .value(py)
                .getattr("message")
                .expect("message")
                .extract()
                .expect("message str");
            assert!(message.contains("outcomes JSON"), "{message}");
            assert!(!message.contains("encode refused"), "{message}");
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

    #[test]
    fn pyfunctions_accept_pathlike_and_return_paths() {
        let root = temp_dir("pyfn");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        write_questions(&repo, "Path?");
        Python::initialize();
        Python::attach(|py| {
            let built = build_request(
                py,
                PathBuf::from("check.toml"),
                Some(repo.clone()),
                "WORKTREE",
            )
            .expect("build_request");
            let digest: String = built
                .bind(py)
                .get_item("prompt_sha256")
                .expect("digest")
                .extract()
                .expect("str");
            assert_eq!(digest.len(), 64);

            let result = run_check(
                py,
                PathBuf::from("check.toml"),
                None,
                true,
                Some(repo.clone()),
                "WORKTREE",
                None,
                None,
                None,
            )
            .expect("dry run");
            assert_eq!(result.prompt_sha256(), digest);
            assert!(result.session().is_none());
            assert!(result.equal_input(py).is_none(py));
            let error = result.ask(py, "why?").expect_err("dry-run ask");
            let code: String = error
                .value(py)
                .getattr("code")
                .expect("code")
                .extract()
                .expect("str");
            assert_eq!(code, "CONFIG_INVALID");

            let logs = root.path().join("log");
            let live = testing::run_check_scripted(
                py,
                PathBuf::from("check.toml"),
                py.eval(c"{'relevant': 'yes'}", None, None)
                    .expect("answers"),
                Some(vec!["anthropic_haiku".to_owned()]),
                false,
                Some(repo.clone()),
                "WORKTREE",
                Some(logs.clone()),
                Some(root.path().join("sessions")),
                None,
            )
            .expect("scripted run");
            let outcomes = live.outcomes(py);
            let backend: String = outcomes
                .bind(py)
                .get_item(0)
                .and_then(|outcome| outcome.get_item("backend"))
                .expect("backend")
                .extract()
                .expect("str");
            assert_eq!(backend, "anthropic_haiku");
            assert!(outcomes.is(live.outcomes(py)), "converted once");
            let session = live.session().expect("session");
            assert_eq!(Some(session.session_id()), live.session_id());
            session.ask(py, "why?").expect("session ask");
            live.ask(py, "and?").expect("result ask");
            assert!(live.__repr__().contains("CheckResult"));
            assert!(session.__repr__().contains("Session"));
            assert!(!live.payload().is_empty());

            assert_eq!(
                log_file(logs.clone()),
                logs.join("logs").join("sc-jev-py.log.jsonl")
            );
            assert_eq!(resolve_log_root(Some(logs.clone())), logs);
            assert_eq!(
                resolve_session_dir(logs.clone(), Some(root.path().join("s"))),
                root.path().join("s")
            );
            assert!(default_log_root().ends_with("log"));
            let bytes = canonical_bytes(py, "{\"b\":1,\"a\":2}").expect("bytes");
            assert_eq!(bytes.bind(py).as_bytes(), b"{\"a\":2,\"b\":1}");
            assert_eq!(
                canonical_sha256(py, "{}").expect("sha"),
                Sha256Digest::of(b"{}").to_string()
            );
            assert!(canonical_sha256(py, "{").is_err());
        });
    }

    /// Stops and joins the Python worker even when an assertion panics.
    /// Only the Unix GIL test (which needs `sh`) uses it.
    #[cfg(unix)]
    struct WorkerGuard(Py<pyo3::types::PyDict>);

    #[cfg(unix)]
    impl Drop for WorkerGuard {
        fn drop(&mut self) {
            Python::attach(|py| {
                let _ = py.run(
                    c"stop.set()\nworker.join(30)\n",
                    Some(self.0.bind(py)),
                    None,
                );
            });
        }
    }

    /// `build_request` releases the GIL while a command extractor runs.
    ///
    /// Sentinel handshake (FTQ-007), no rate or tick count: the extractor
    /// writes `started`, then polls up to 20 s for `ack`. A Python worker
    /// thread waits for `started` and writes `ack`; it can only run if the
    /// GIL is free. With `ack` the extractor prints a non-subjects document
    /// (`EXTRACTION_INVALID`); without it the extractor exits 3
    /// (`EXTRACTION_FAILED`). The extractor timeout (60 s) is above both
    /// bounds.
    #[cfg(unix)]
    #[test]
    fn build_request_releases_the_gil_during_a_slow_extractor() {
        let root = temp_dir("gil");
        let repo = root.path();
        let started = repo.join("started");
        let ack = repo.join("ack");
        write_questions(repo, "Slow:\n\n{slow}");
        let script = "touch \"$1\"; i=0; while [ ! -e \"$2\" ]; do i=$((i+1)); [ $i -gt 400 ] && exit 3; sleep 0.05; done; echo not-subjects";
        let command = serde_json::to_string(&[
            "sh",
            "-c",
            script,
            "sh",
            started.to_str().expect("utf8 path"),
            ack.to_str().expect("utf8 path"),
        ])
        .expect("argv");
        std::fs::write(
            repo.join("check.toml"),
            format!(
                "[questions]\npath = \"questions.json\"\n\n[inputs.slow]\nkind = \"command\"\ncommand = {command}\ntimeout_seconds = 60\n"
            ),
        )
        .expect("check");
        Python::initialize();
        Python::attach(|py| {
            let scope = pyo3::types::PyDict::new(py);
            scope
                .set_item("started", started.to_str().expect("utf8 path"))
                .expect("started");
            scope
                .set_item("ack", ack.to_str().expect("utf8 path"))
                .expect("ack");
            py.run(
                c"import os, threading, time\nstop = threading.Event()\ndef answer():\n    deadline = time.monotonic() + 20\n    while not stop.is_set() and time.monotonic() < deadline:\n        if os.path.exists(started):\n            open(ack, 'w').close()\n            return\n        time.sleep(0.01)\nworker = threading.Thread(target=answer)\nworker.start()\n",
                Some(&scope),
                None,
            )
            .expect("start thread");
            let _guard = WorkerGuard(scope.clone().unbind());
            let error = build_request(
                py,
                PathBuf::from("check.toml"),
                Some(repo.to_path_buf()),
                "WORKTREE",
            )
            .expect_err("extractor output is not a subjects document");
            let code: String = error
                .value(py)
                .getattr("code")
                .expect("code")
                .extract()
                .expect("str");
            assert_eq!(
                code, "EXTRACTION_INVALID",
                "EXTRACTION_FAILED means the worker never ran: the GIL was held"
            );
        });
    }
}
