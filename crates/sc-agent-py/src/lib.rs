//! The `sc_agent` Python module: one function per decidr operation (ANC-006).
//!
//! `send` (`decidr send`), `send_batch` (`decidr send --batch`), `models` (`decidr models`) and
//! `doctor` (`decidr doctor`) call the decidr library (ADR-SAG-0004) and return the same JSON
//! text the CLI prints. Strings are parsed at the edge (`ModelSelector::parse`,
//! `Provider::from_str`, `Slug::new`); every error raises `SagError(id, message, detail)`.
//!
//! The decidr call runs on a scoped worker thread; the Python thread waits on its result
//! channel with a 100 ms timed wait outside the GIL and calls `check_signals` between waits,
//! so `KeyboardInterrupt` cancels the run, drains it and is re-raised (REQ-SAG-0011).
//! `general_log` takes a caller-owned `sc_observability.Logger` or `Callable[[dict], None]`
//! (REQ-SAG-0008); the session log is always the default file. Configuration uses the shared
//! decidr layers.

mod binding_error;
mod keyring_options;

use binding_error::PyBindingError;

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use decidr::input::{BatchInput, build_batch, build_request_bytes};
use decidr::{
    Context, DecidrError, DoctorOptions, ModelSelector, ModelsOptions, SendOptions, general_logger,
    load_config, session_logger,
};
use pyo3::exceptions::{PyRuntimeError, PyRuntimeWarning, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use sc_agent_config::RunnerName;
use sc_agent_log::{GeneralLogger, Injected, SessionLogger, Slug};
use sc_agent_types::{ErrorId, Provider};
use sc_decision_schema::DecisionResult;
use sc_decision_schema::{Answer, SystemOneResponse};
use serde_json::Value;

/// How long the Python thread waits on the result channel before it checks for signals.
const SIGNAL_POLL: Duration = Duration::from_millis(100);

fn parse_detail(detail: &str) -> Option<sc_agent_types::ErrorDetail> {
    serde_json::from_str(detail).ok()
}

#[pyfunction]
fn category(detail: &str) -> &'static str {
    parse_detail(detail)
        .and_then(|detail| detail.category)
        .unwrap_or(sc_agent_types::ErrorCategory::Other)
        .as_str()
}

#[pyfunction]
fn retryable(detail: &str) -> bool {
    parse_detail(detail)
        .and_then(|detail| detail.category)
        .unwrap_or(sc_agent_types::ErrorCategory::Other)
        .retryable()
}

/// Parse once for SagError construction; properties read only the returned values.
#[pyfunction]
fn _sag_error_fields(detail: &str) -> (Option<u16>, Option<u32>, Option<u64>, &'static str, bool) {
    let detail = parse_detail(detail);
    let category = detail
        .as_ref()
        .and_then(|detail| detail.category)
        .unwrap_or(sc_agent_types::ErrorCategory::Other);
    (
        detail.as_ref().and_then(|detail| detail.status),
        detail.as_ref().and_then(|detail| detail.attempts),
        detail.as_ref().and_then(|detail| detail.retry_after_ms),
        category.as_str(),
        category.retryable(),
    )
}

fn make_sag_error(id: &str, message: String, detail: String) -> PyErr {
    Python::attach(|py| {
        py.import("sc_agent")
            .and_then(|module| module.getattr("SagError"))
            .and_then(|class| class.call1((id, message, detail)))
            .map_or_else(|error| error, PyErr::from_value)
    })
}

/// Construct the exception from the real typed error detail.
fn sag_error(error: &DecidrError) -> PyErr {
    let detail = serde_json::to_string(&error.detail()).unwrap_or_else(|_| "{}".to_owned());
    make_sag_error(error.id(), error.message(), detail)
}

/// A cancel token shared with the decidr call (REQ-SAG-0104).
#[pyclass(module = "sc_agent", frozen)]
pub struct CancelToken(sc_agent_types::CancelToken);

#[pymethods]
impl CancelToken {
    /// A token that is not cancelled.
    #[new]
    fn new() -> Self {
        Self(sc_agent_types::CancelToken::new())
    }

    /// Cancels every request of every call holding this token.
    fn cancel(&self) {
        self.0.cancel();
    }

    /// Whether `cancel` was called.
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

#[pyclass(module = "sc_agent", frozen)]
pub struct NoulAnswer {
    #[pyo3(get)]
    noul: f64,
}
#[pymethods]
impl NoulAnswer {
    #[new]
    fn new(noul: f64) -> Self {
        Self { noul }
    }
}

#[pyclass(module = "sc_agent", frozen)]
pub struct ChoiceAnswer {
    #[pyo3(get)]
    choice: String,
    #[pyo3(get)]
    confidence: f64,
    #[pyo3(get)]
    probabilities: Py<PyAny>,
}
#[pymethods]
impl ChoiceAnswer {
    #[new]
    fn new(
        py: Python<'_>,
        choice: String,
        confidence: f64,
        probabilities: Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let probabilities = py
            .import("types")?
            .getattr("MappingProxyType")?
            .call1((probabilities,))?
            .unbind();
        Ok(Self {
            choice,
            confidence,
            probabilities,
        })
    }
}

#[pyclass(module = "sc_agent", frozen)]
pub struct ScoreAnswer {
    #[pyo3(get)]
    score: f64,
    #[pyo3(get)]
    confidence: f64,
    #[pyo3(get)]
    legend: Py<PyAny>,
    #[pyo3(get)]
    probabilities: Py<PyAny>,
}
#[pymethods]
impl ScoreAnswer {
    #[new]
    fn new(
        py: Python<'_>,
        score: f64,
        confidence: f64,
        legend: Bound<'_, PyAny>,
        probabilities: Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mapping_proxy = py.import("types")?.getattr("MappingProxyType")?;
        let legend = mapping_proxy.call1((legend,))?.unbind();
        let probabilities = mapping_proxy.call1((probabilities,))?.unbind();
        Ok(Self {
            score,
            confidence,
            legend,
            probabilities,
        })
    }
}

#[pyclass(module = "sc_agent", frozen)]
pub struct BoxAnswer {
    #[pyo3(get)]
    top: f64,
    #[pyo3(get)]
    bottom: f64,
    #[pyo3(get)]
    left: f64,
    #[pyo3(get)]
    right: f64,
    #[pyo3(get)]
    confidence: Option<f64>,
}
#[pymethods]
impl BoxAnswer {
    #[new]
    fn new(top: f64, bottom: f64, left: f64, right: f64, confidence: Option<f64>) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
            confidence,
        }
    }
}

fn json_object(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    let encoded =
        serde_json::to_string(value).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(py
        .import("json")?
        .call_method1("loads", (encoded,))?
        .unbind())
}

#[pyfunction]
fn parse_result(py: Python<'_>, line: &str) -> PyResult<Py<PyAny>> {
    let parsed: Value = serde_json::from_str(line)
        .map_err(|e| PyTypeError::new_err(format!("invalid result JSON: {e}")))?;
    let row = parsed
        .as_object()
        .ok_or_else(|| PyTypeError::new_err("result must be an object"))?;
    let result = PyDict::new(py);
    for field in ["session_id", "position", "sends", "digests"] {
        let value = row
            .get(field)
            .ok_or_else(|| PyTypeError::new_err(format!("result is missing {field}")))?;
        result.set_item(field, json_object(py, value)?)?;
    }
    if let Some(response) = row.get("response") {
        let encoded =
            serde_json::to_vec(response).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let response = SystemOneResponse::from_json(200, &encoded).map_err(|e| {
            PyTypeError::new_err(format!("invalid typed response: {}", e.message()))
        })?;
        let answers = PyDict::new(py);
        for (id, answer) in response.answers.iter() {
            let typed = match answer {
                Answer::Noul { noul } => Py::new(py, NoulAnswer { noul: *noul })?.into_any(),
                Answer::Choice {
                    choice,
                    confidence,
                    probabilities,
                } => {
                    let probs = json_object(
                        py,
                        &serde_json::to_value(probabilities)
                            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
                    )?;
                    let probs = py
                        .import("types")?
                        .getattr("MappingProxyType")?
                        .call1((probs.bind(py),))?
                        .unbind();
                    Py::new(
                        py,
                        ChoiceAnswer {
                            choice: choice.clone(),
                            confidence: *confidence,
                            probabilities: probs,
                        },
                    )?
                    .into_any()
                }
                Answer::Score {
                    score,
                    confidence,
                    legend,
                    probabilities,
                } => {
                    let legend = json_object(
                        py,
                        &serde_json::to_value(legend)
                            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
                    )?;
                    let probs = json_object(
                        py,
                        &serde_json::to_value(probabilities)
                            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
                    )?;
                    let legend = py
                        .import("types")?
                        .getattr("MappingProxyType")?
                        .call1((legend.bind(py),))?
                        .unbind();
                    let probs = py
                        .import("types")?
                        .getattr("MappingProxyType")?
                        .call1((probs.bind(py),))?
                        .unbind();
                    Py::new(
                        py,
                        ScoreAnswer {
                            score: *score,
                            confidence: *confidence,
                            legend,
                            probabilities: probs,
                        },
                    )?
                    .into_any()
                }
                Answer::Box {
                    coordinates,
                    confidence,
                } => Py::new(
                    py,
                    BoxAnswer {
                        top: coordinates.top,
                        bottom: coordinates.bottom,
                        left: coordinates.left,
                        right: coordinates.right,
                        confidence: *confidence,
                    },
                )?
                .into_any(),
                _ => return Err(PyTypeError::new_err("unsupported answer variant")),
            };
            answers.set_item(id, typed)?;
        }
        result.set_item("answers", answers)?;
        result.set_item(
            "usage",
            json_object(
                py,
                &serde_json::to_value(response.usage)
                    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
            )?,
        )?;
        result.set_item("error", py.None())?;
    } else if let Some(error) = row.get("error") {
        let encoded =
            serde_json::to_string(error).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let detail: sc_agent_types::ErrorDetail = serde_json::from_str(&encoded)
            .map_err(|e| PyTypeError::new_err(format!("invalid typed error: {e}")))?;
        let module = py.import("sc_agent")?;
        let exception = module
            .getattr("SagError")?
            .call1((detail.id, detail.message, encoded))?;
        result.set_item("error", exception)?;
        result.set_item("answers", py.None())?;
        result.set_item("usage", py.None())?;
    } else {
        return Err(PyTypeError::new_err(
            "result has neither response nor error",
        ));
    }
    Ok(result.into_any().unbind())
}

/// A batch item: a JSON request string, or `(request, images, long_context)`.
#[derive(FromPyObject)]
enum BatchItem {
    /// A bare Jev request.
    Bare(String),
    /// A request with image bytes by name and the long-context flag.
    Full(String, Option<BTreeMap<String, Vec<u8>>>, bool),
}

impl From<BatchItem> for BatchInput {
    fn from(item: BatchItem) -> Self {
        match item {
            BatchItem::Bare(json) => Self {
                json,
                images: Vec::new(),
                long_context: false,
            },
            BatchItem::Full(json, images, long_context) => Self {
                json,
                images: images.unwrap_or_default().into_iter().collect(),
                long_context,
            },
        }
    }
}

/// What a worker's call gives back: its result, and any logger that failed to close.
type Outcome<T> = (Result<T, DecidrError>, Vec<DecidrError>);

/// What the worker sends the Python thread.
enum Message<T> {
    /// One result line, as it finished.
    Line(u64, String),
    /// The decidr call returned and its loggers were closed.
    Done(Outcome<T>),
}

/// The callback a decidr call reports each result to.
type OnResult<'a> = &'a mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>;

fn py_binding_error(error: &PyBindingError) -> PyErr {
    let detail = serde_json::to_string(&error.detail()).unwrap_or_else(|_| "{}".to_owned());
    make_sag_error(error.id(), error.message(), detail)
}

/// `SagError(sc_agent_py.worker_panicked, message, detail)`.
fn worker_panicked(message: String) -> PyErr {
    py_binding_error(&PyBindingError::WorkerPanicked { message })
}

/// The text of a panic payload (`&str` or `String`), else a fixed note.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the worker panicked with a non-text payload".to_owned())
}

/// Runs `work` on a scoped worker thread while the Python thread waits for its results.
///
/// The wait is `recv_timeout(100 ms)` outside the GIL, then `check_signals` with it. A signal
/// exception (`KeyboardInterrupt`) or an `on_result` exception cancels `cancel`, drains the
/// remaining results, joins the worker and is re-raised (REQ-SAG-0011). After a signal the
/// drained results are still passed to `on_result`; after an `on_result` exception they are
/// not. A logger that failed to close (or a session flush after the result) is reported as a
/// `RuntimeWarning` whose text is the error chain as JSON; it never changes the
/// result (REQ-SAG-0008). A worker panic raises `SagError` `sc_agent_py.worker_panicked`.
fn run_interruptible<T: Send>(
    py: Python<'_>,
    cancel: &sc_agent_types::CancelToken,
    on_result: Option<&Py<PyAny>>,
    work: impl FnOnce(OnResult<'_>) -> Outcome<T> + Send,
) -> PyResult<(T, Vec<String>)> {
    let (sender, receiver) = mpsc::channel::<Message<T>>();
    // `py.detach` runs its closure without the GIL and requires it to be `Send` (pyo3's
    // `Ungil`). `Receiver` is `Send` but not `Sync`, so `&Receiver` is not `Send`; the Mutex
    // makes the shared reference `Send`. Only this thread ever locks it, so it never contends.
    let receiver = Mutex::new(receiver);
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            let lines = sender.clone();
            let mut forward = |result: DecisionResult| {
                let position = result.position;
                let text = decidr::output::result_text(&result)?;
                // The receiver lives until the worker is joined, so a send cannot fail.
                let _ = lines.send(Message::Line(position, text));
                Ok(())
            };
            let outcome = work(&mut forward);
            let _ = sender.send(Message::Done(outcome));
        });

        let mut lines = Vec::new();
        let mut raised: Option<PyErr> = None;
        let mut callback_failed = false;
        let outcome = loop {
            let message = py.detach(|| {
                receiver
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .recv_timeout(SIGNAL_POLL)
            });
            match message {
                Ok(Message::Line(position, line)) => {
                    if let Some(callback) = on_result.filter(|_| !callback_failed)
                        && let Err(error) = callback.call1(py, (line.clone(),))
                    {
                        callback_failed = true;
                        cancel.cancel();
                        raised.get_or_insert(error);
                    }
                    lines.push((position, line));
                }
                Ok(Message::Done(outcome)) => break Some(outcome),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break None,
            }
            if raised.is_none()
                && let Err(error) = py.check_signals()
            {
                cancel.cancel();
                raised = Some(error);
            }
        };
        lines.sort_by_key(|(position, _)| *position);
        let lines: Vec<String> = lines.into_iter().map(|(_, line)| line).collect();
        let joined = py.detach(move || worker.join());
        if let Err(payload) = &joined {
            return Err(worker_panicked(panic_message(&**payload)));
        }
        let Some((result, closed)) = outcome else {
            return Err(worker_panicked(
                "the worker ended without a result".to_owned(),
            ));
        };
        for error in &closed {
            // The whole chain (every cause id, e.g. `log.rows_lost`), as `SagError`'s detail.
            let text =
                decidr::output::to_json(&error.detail()).unwrap_or_else(|_| error.to_string());
            let message = std::ffi::CString::new(text).unwrap_or_default();
            if let Err(warning) = PyErr::warn(py, &py.get_type::<PyRuntimeWarning>(), &message, 1) {
                // A warning filter must not replace the decision (or its error).
                warning.write_unraisable(py, None);
            }
        }
        if let Some(error) = raised {
            return Err(error);
        }
        result
            .map(|value| (value, lines))
            .map_err(|error| sag_error(&error))
    })
}

/// Resolve the decidr layers without opening files for logging.
fn context(general_log: Option<Py<PyAny>>, project: Option<&str>) -> Result<Context, DecidrError> {
    let mut context = Context::new(sc_agent_config::CliName::Decidr);
    if let Some(project) = project {
        context.flags.insert("project".into(), project.into());
    }
    context.general = general_log.map(|target| Injected::Python(Arc::new(target)));
    context.loaded = Some(decidr::load_context(&context)?);
    Ok(context)
}

fn open_general(context: &mut Context) -> Result<(), DecidrError> {
    context.opened_general = Some(general_logger(context)?);
    Ok(())
}

/// Parse the required role and resolve project before opening either logger.
fn send_options(
    model: &str,
    project: Option<&str>,
    agent_role: &str,
    provider: ProviderOption<'_>,
    general_log: Option<Py<PyAny>>,
) -> Result<SendOptions, DecidrError> {
    let agent_role = Slug::new("agent_role", agent_role)?;
    let mut context = context(general_log, project)?;
    let config = load_config(&context)?;
    let project = Slug::new("project", config.require_project()?)?;
    let model = ModelSelector::parse(model, &config)?;
    let provider = provider.parse()?;
    open_general(&mut context)?;
    let mut options = SendOptions {
        model,
        provider,
        project,
        agent_role,
        context,
        session: None,
    };
    options.session = Some(session_logger(&options)?);
    Ok(options)
}

enum ProviderOption<'a> {
    Text(Option<&'a str>),
}

impl ProviderOption<'_> {
    fn parse(self) -> Result<Option<Provider>, sc_agent_types::TypesError> {
        match self {
            Self::Text(provider) => provider.map(str::parse::<Provider>).transpose(),
        }
    }
}

/// Closes the loggers of an operation on the worker thread, after the operation, so every
/// record and row is written (or with its callable) before the call returns.
///
/// A callable that raised made its writes refused writes, which `close` reports as lost rows;
/// they are returned as warnings and never change the result (REQ-SAG-0008).
fn close_loggers(
    general: Option<GeneralLogger>,
    session: Option<SessionLogger>,
) -> Vec<DecidrError> {
    let mut failed = Vec::new();
    if let Some(Err(error)) = session.map(SessionLogger::close) {
        failed.push(DecidrError::from(error));
    }
    if let Some(Err(error)) = general.map(GeneralLogger::close) {
        failed.push(DecidrError::from(error));
    }
    failed
}

fn token(cancel: Option<&CancelToken>) -> sc_agent_types::CancelToken {
    cancel.map_or_else(sc_agent_types::CancelToken::new, |token| token.0.clone())
}

/// Parses provider-scoped keyring overrides before opening loggers or sending.
fn keyring_override(
    service: Option<&str>,
    account: Option<&str>,
) -> PyResult<Option<sc_agent_secrets::KeyringLocation>> {
    keyring_options::location(service, account).map_err(|error| {
        let option = match error {
            sc_agent_secrets::SecretsError::InvalidLocation { option } => option,
            _ => "keyring",
        };
        invalid_option(option, "must contain 1..=255 UTF-8 bytes")
    })
}

fn invalid_option(option: &str, bound: &str) -> PyErr {
    py_binding_error(&PyBindingError::InvalidOption {
        option: option.to_owned(),
        bound: bound.to_owned(),
    })
}

/// `decidr send`: one request, always `Decision::decide`; returns its result line (JSON).
///
/// `images` maps part names to PNG or JPEG bytes. Raises `SagError` for a command error; a
/// failed decision is data in the returned line.
#[pyfunction]
#[pyo3(signature = (request, *, model, project = None, agent_role, provider = None, images = None, long_context = false, cancel = None, general_log = None, keyring_service = None, keyring_account = None, timeout_secs = None))]
#[expect(
    clippy::too_many_arguments,
    reason = "one keyword argument per decidr send flag (ANC-006)"
)]
fn send(
    py: Python<'_>,
    request: &str,
    model: &str,
    project: Option<&str>,
    agent_role: &str,
    provider: Option<&str>,
    images: Option<BTreeMap<String, Vec<u8>>>,
    long_context: bool,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
    keyring_service: Option<&str>,
    keyring_account: Option<&str>,
    timeout_secs: Option<f64>,
) -> PyResult<String> {
    let timeout = timeout_secs
        .map(decidr::RequestTimeout::from_secs)
        .transpose()
        .map_err(|error| sag_error(&error))?;
    let keyring = keyring_override(keyring_service, keyring_account)?;
    let provider = ProviderOption::Text(provider);
    // The request is checked before any logger opens, so a refused one writes no file.
    let images = images.unwrap_or_default().into_iter().collect();
    let request = build_request_bytes(request.as_bytes(), images, long_context, None)
        .map_err(|error| sag_error(&error))?;
    let mut options = send_options(model, project, agent_role, provider, general_log)
        .map_err(|error| sag_error(&error))?;
    options.context.keyring_override = keyring;
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let (line, _) = run_interruptible(py, &cancel, None, move |_| {
        let mut options = options;
        let mut line = None;
        let sent = decidr::send_with(
            &options,
            request,
            &worker_cancel,
            &decidr::CallLimits {
                timeout,
                workers: None,
            },
            &mut |result| {
                line = Some(decidr::output::result_text(&result)?);
                Ok(())
            },
        );
        let (general, session) = (
            options.context.opened_general.take(),
            options.session.take(),
        );
        drop(options);
        let mut closed = close_loggers(general, session);
        // A result that exists is returned; a flush failure after it is a warning, never a
        // reason to drop the result (QA-006).
        match (line, sent) {
            (Some(line), Err(error)) => {
                closed.insert(0, error);
                (Ok(line), closed)
            }
            (Some(line), Ok(())) => (Ok(line), closed),
            (None, Err(error)) => (Err(error), closed),
            (None, Ok(())) => (
                Err(DecidrError::from(
                    sc_decision_schema::DecisionError::Shutdown {
                        reason: "decidr::send returned without a result".to_owned(),
                    },
                )),
                closed,
            ),
        }
    })?;
    Ok(line)
}

/// `decidr send --batch`: always `Decision::decide_stream`; returns the result lines in
/// input order and passes each to `on_result` as it arrives.
#[pyfunction]
#[pyo3(signature = (requests, *, model, project = None, agent_role, provider = None, cancel = None, on_result = None, general_log = None, keyring_service = None, keyring_account = None, timeout_secs = None, workers = None))]
#[expect(
    clippy::too_many_arguments,
    reason = "one keyword argument per decidr send flag (ANC-006)"
)]
fn send_batch(
    py: Python<'_>,
    requests: Vec<BatchItem>,
    model: &str,
    project: Option<&str>,
    agent_role: &str,
    provider: Option<&str>,
    cancel: Option<&CancelToken>,
    on_result: Option<Py<PyAny>>,
    general_log: Option<Py<PyAny>>,
    keyring_service: Option<&str>,
    keyring_account: Option<&str>,
    timeout_secs: Option<f64>,
    workers: Option<i64>,
) -> PyResult<Vec<String>> {
    let workers = workers
        .map(decidr::Workers::new)
        .transpose()
        .map_err(|error| sag_error(&error))?;
    let timeout = timeout_secs
        .map(decidr::RequestTimeout::from_secs)
        .transpose()
        .map_err(|error| sag_error(&error))?;
    let keyring = keyring_override(keyring_service, keyring_account)?;
    let provider = ProviderOption::Text(provider);
    let requests = build_batch(requests.into_iter().map(BatchInput::from).collect())
        .map_err(|error| sag_error(&error))?;
    let mut options = send_options(model, project, agent_role, provider, general_log)
        .map_err(|error| sag_error(&error))?;
    options.context.keyring_override = keyring;
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let ((), lines) = run_interruptible(py, &cancel, on_result.as_ref(), move |forward| {
        let mut options = options;
        let outcome = decidr::send_batch_with(
            &options,
            requests,
            &worker_cancel,
            &decidr::CallLimits { timeout, workers },
            forward,
        );
        let (general, session) = (
            options.context.opened_general.take(),
            options.session.take(),
        );
        drop(options);
        (outcome, close_loggers(general, session))
    })?;
    Ok(lines)
}

/// `decidr models`: the TypeSafe models list, or a configured runner's (JSON).
#[pyfunction]
#[pyo3(signature = (runner = None, cancel = None, general_log = None))]
fn models(
    py: Python<'_>,
    runner: Option<&str>,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
) -> PyResult<String> {
    let runner = runner
        .map(runner_name)
        .transpose()
        .map_err(|error| sag_error(&error))?;
    let options = ModelsOptions {
        runner,
        context: {
            let mut context = context(general_log, None).map_err(|error| sag_error(&error))?;
            open_general(&mut context).map_err(|error| sag_error(&error))?;
            context
        },
    };
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let (text, _) = run_interruptible(py, &cancel, None, move |_| {
        let mut options = options;
        let list = decidr::models(&options, &worker_cancel);
        let general = options.context.opened_general.take();
        drop(options);
        let closed = close_loggers(general, None);
        (list.and_then(|list| decidr::output::to_json(&list)), closed)
    })?;
    Ok(text)
}

/// `runner` as a `RunnerName`, parsed once at the edge; a name that cannot be a runner name
/// is no configured runner either (`decidr.unknown_runner`).
fn runner_name(runner: &str) -> Result<RunnerName, DecidrError> {
    RunnerName::try_from(runner.to_owned()).or_else(|_| {
        let probe = context(None, None)?;
        let config = load_config(&probe)?;
        Err(DecidrError::UnknownRunner {
            name: runner.to_owned(),
            known: config.local_runners.keys().cloned().collect(),
        })
    })
}

/// `decidr doctor`: detected keys, missing provider models and config ties (JSON).
#[pyfunction]
#[pyo3(signature = (cancel = None, general_log = None))]
fn doctor(
    py: Python<'_>,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
) -> PyResult<String> {
    let options = DoctorOptions {
        context: context(general_log, None).map_err(|error| sag_error(&error))?,
    };
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let (text, _) = run_interruptible(py, &cancel, None, move |_| {
        let mut options = options;
        let report = decidr::doctor(&options, &worker_cancel);
        let general = options.context.opened_general.take();
        drop(options);
        let closed = close_loggers(general, None);
        (
            report.and_then(|report| decidr::output::to_json(&report)),
            closed,
        )
    })?;
    Ok(text)
}

/// Resolve configuration only: no provider calls or logger creation.
#[pyfunction]
#[pyo3(signature = (*, project = None))]
fn resolved_config(py: Python<'_>, project: Option<&str>) -> PyResult<Py<PyAny>> {
    let context = context(None, project).map_err(|error| sag_error(&error))?;
    let loaded = decidr::load_context(&context).map_err(|error| sag_error(&error))?;
    let result = PyDict::new(py);
    result.set_item("project", loaded.config.project())?;
    let sources = PyDict::new(py);
    for (key, source) in loaded.sources {
        sources.set_item(key, source.to_string())?;
    }
    result.set_item("sources", sources)?;
    Ok(result.into_any().unbind())
}

/// The default log root, `~/.sc-agent`, where a generated client opens its logger.
#[pyfunction]
fn default_log_root() -> PyResult<std::path::PathBuf> {
    sc_agent_types::cli_log_root(None, sc_agent_types::resolve_home())
        .ok_or_else(|| sag_error(&sc_agent_config::AgentConfigError::HomeUnknown.into()))
}

/// The `sc_agent` module: `send`, `send_batch`, `models`, `doctor`, `CancelToken`, `SagError`.
#[pymodule(name = "_native")]
fn sc_agent(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(send, module)?)?;
    module.add_function(wrap_pyfunction!(send_batch, module)?)?;
    module.add_function(wrap_pyfunction!(category, module)?)?;
    module.add_function(wrap_pyfunction!(retryable, module)?)?;
    module.add_function(wrap_pyfunction!(_sag_error_fields, module)?)?;
    module.add_function(wrap_pyfunction!(models, module)?)?;
    module.add_function(wrap_pyfunction!(doctor, module)?)?;
    module.add_function(wrap_pyfunction!(resolved_config, module)?)?;
    module.add_function(wrap_pyfunction!(default_log_root, module)?)?;
    module.add_function(wrap_pyfunction!(parse_result, module)?)?;
    module.add_class::<CancelToken>()?;
    module.add_class::<NoulAnswer>()?;
    module.add_class::<ChoiceAnswer>()?;
    module.add_class::<ScoreAnswer>()?;
    module.add_class::<BoxAnswer>()?;
    module.py().run(
        cr#"
class SagError(Exception):
    def __init__(self, id, message, detail_json):
        super().__init__(id, message, detail_json)
        self._fields = _sag_error_fields(detail_json)
    def __str__(self):
        return f"{self.id}: {self.message}"
    @property
    def id(self): return self.args[0]
    @property
    def message(self): return self.args[1]
    @property
    def detail(self): return self.args[2]
    @property
    def status(self): return self._fields[0]
    @property
    def attempts(self): return self._fields[1]
    @property
    def retry_after_ms(self): return self._fields[2]
    @property
    def category(self): return self._fields[3]
    @property
    def retryable(self): return self._fields[4]
"#,
        Some(&module.dict()),
        None,
    )?;
    module.add("SagError", module.getattr("SagError")?)?;
    Ok(())
}
