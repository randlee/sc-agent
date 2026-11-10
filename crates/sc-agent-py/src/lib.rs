//! The `sc_agent` Python module: one function per decide operation (ANC-006).
//!
//! `send` (`decide send`), `send_batch` (`decide send --batch`), `models` (`decide models`) and
//! `doctor` (`decide doctor`) call the decide library (ADR-SAG-0004) and return the same JSON
//! text the CLI prints. Strings are parsed at the edge (`ModelSelector::parse`,
//! `Provider::from_str`, `Slug::new`); every error raises `SagError(id, message, detail)`.
//!
//! The decide call runs on a scoped worker thread; the Python thread waits on its result
//! channel with a 100 ms timed wait outside the GIL and calls `check_signals` between waits,
//! so `KeyboardInterrupt` cancels the run, drains it and is re-raised (REQ-SAG-0011).
//! `general_log` and `session_log` take `Callable[[dict], None]` (REQ-SAG-0008). `config_dir`
//! (a path) names the configuration directory; absent, it is `~/.sc-agent`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use decide::input::{BatchInput, build_batch, build_request_bytes};
use decide::{
    Context, DecideError, DoctorOptions, ModelSelector, ModelsOptions, SendOptions, general_logger,
    load_config, session_identity, session_logger,
};
use pyo3::exceptions::{PyException, PyRuntimeWarning};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use sc_agent_config::RunnerName;
use sc_agent_log::{GeneralLogger, LogError, LogKind, SessionLogger, Slug};
use sc_agent_types::{ErrorId, Provider};
use sc_decision_schema::DecisionResult;
use sc_observability::v2::{
    LogEvent, LogSink, LogSinkError, Logger, LoggerBuilder, LoggerConfig, SinkRegistration,
};
use sc_observability::{
    ErrorContext, Remediation, ServiceName, SinkHealth, SinkHealthState, SinkName,
};
use serde_json::{Map, Value};

/// The general-log service of sc-agent-py.
const SERVICE: &str = "sc-agent-py";

/// How long the Python thread waits on the result channel before it checks for signals.
const SIGNAL_POLL: Duration = Duration::from_millis(100);

pyo3::create_exception!(
    sc_agent,
    SagError,
    PyException,
    "An sc-agent error; `args` is `(id, message, detail)`, `detail` the JSON error chain."
);

/// `SagError(id, message, detail JSON)` for `error`; `detail` carries every cause and recovery.
fn sag_error(error: &DecideError) -> PyErr {
    let detail = serde_json::to_string(&error.detail()).unwrap_or_else(|_| "{}".to_owned());
    SagError::new_err((error.id(), error.message(), detail))
}

/// A cancel token shared with the decide call (REQ-SAG-0104).
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

/// Calls a Python `Callable[[dict], None]` with each record under the GIL.
///
/// The dict is `{"level", "target", "action", "fields"}`. A raised exception makes the
/// write a refused write (counted by sc-agent-log, reported by its flush); it never changes a
/// decision result.
struct PyCallableSink {
    callable: Py<PyAny>,
    name: SinkName,
}

impl PyCallableSink {
    fn record(event: &LogEvent) -> Value {
        let mut record = Map::new();
        record.insert(
            "level".into(),
            serde_json::to_value(event.level).unwrap_or(Value::Null),
        );
        record.insert("target".into(), event.target.as_str().into());
        record.insert("action".into(), event.action.as_str().into());
        record.insert("fields".into(), Value::Object(event.fields.clone()));
        Value::Object(record)
    }

    fn refused(message: String) -> LogSinkError {
        LogSinkError::Write {
            context: Box::new(ErrorContext::new(
                sc_observability::error_codes::LOGGER_SINK_WRITE_FAILED,
                message,
                Remediation::not_recoverable("the Python log callable raised; fix the callable"),
            )),
        }
    }
}

impl LogSink for PyCallableSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        let text = Self::record(event).to_string();
        Python::attach(|py| {
            let record = py
                .import("json")
                .and_then(|json| json.call_method1("loads", (text,)))?;
            self.callable.call1(py, (record,)).map(drop)
        })
        .map_err(|error| Self::refused(format!("the log callable raised: {error}")))
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: self.name.clone(),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

/// A logger whose only sink is `callable` (file and console sinks off, as `MemoryLog`).
fn callable_logger(callable: Py<PyAny>, service: &'static str) -> Result<Arc<Logger>, DecideError> {
    let log_kind = if service == SessionLogger::SERVICE {
        LogKind::Session
    } else {
        LogKind::General
    };
    let name_error =
        |what: &'static str, value: &str, error: &(dyn std::error::Error + 'static)| {
            DecideError::from(LogError::InvalidName {
                what,
                value: value.to_owned(),
                cause: sc_agent_types::ErrorDetail::foreign("sc_observability.invalid_name", error),
            })
        };
    let service_name =
        ServiceName::new(service).map_err(|error| name_error("service", service, &error))?;
    let mut config = LoggerConfig::default_for(service_name, PathBuf::new());
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    config.queue_capacity = sc_agent_log::QUEUE_CAPACITY;
    let name = SinkName::new("python").map_err(|error| name_error("sink", "python", &error))?;
    let sink: Arc<dyn LogSink> = Arc::new(PyCallableSink { callable, name });
    let start = |source| {
        DecideError::from(LogError::Start {
            log: log_kind,
            source,
        })
    };
    let mut builder = LoggerBuilder::new(config).map_err(start)?;
    builder.register_sink(SinkRegistration::typed(sink));
    builder.build().map(Arc::new).map_err(start)
}

/// What a worker's call gives back: its result, and any logger that failed to close.
type Outcome<T> = (Result<T, DecideError>, Vec<DecideError>);

/// What the worker sends the Python thread.
enum Message<T> {
    /// One result line, as it finished.
    Line(String),
    /// The decide call returned and its loggers were closed.
    Done(Outcome<T>),
}

/// The callback a decide call reports each result to.
type OnResult<'a> = &'a mut dyn FnMut(DecisionResult) -> Result<(), DecideError>;

/// The stable id of a worker that panicked or ended without a result (RBP-F003).
const WORKER_PANICKED: &str = "sc_agent_py.worker_panicked";

/// `SagError(sc_agent_py.worker_panicked, message, detail)`.
fn worker_panicked(message: String) -> PyErr {
    let detail = sc_agent_types::ErrorDetail::new(WORKER_PANICKED, message.clone(), None);
    let detail = decide::output::to_json(&detail).unwrap_or_else(|_| "{}".to_owned());
    SagError::new_err((WORKER_PANICKED, message, detail))
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
                let text = decide::output::result_text(&result)?;
                // The receiver lives until the worker is joined, so a send cannot fail.
                let _ = lines.send(Message::Line(text));
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
                Ok(Message::Line(line)) => {
                    if let Some(callback) = on_result.filter(|_| !callback_failed)
                        && let Err(error) = callback.call1(py, (line.clone(),))
                    {
                        callback_failed = true;
                        cancel.cancel();
                        raised.get_or_insert(error);
                    }
                    lines.push(line);
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
                decide::output::to_json(&error.detail()).unwrap_or_else(|_| error.to_string());
            let message = std::ffi::CString::new(text).unwrap_or_default();
            PyErr::warn(py, &py.get_type::<PyRuntimeWarning>(), &message, 1)?;
        }
        if let Some(error) = raised {
            return Err(error);
        }
        result
            .map(|value| (value, lines))
            .map_err(|error| sag_error(&error))
    })
}

/// A context for sc-agent-py: `config_dir` (else `~/.sc-agent`), and the general-log
/// callable, else the default general log.
fn context(
    general_log: Option<Py<PyAny>>,
    config_dir: Option<PathBuf>,
) -> Result<Context, DecideError> {
    let mut context = Context::new(SERVICE);
    context.config_dir = config_dir;
    let general = match general_log {
        Some(callable) => GeneralLogger::injected(callable_logger(callable, SERVICE)?),
        None => general_logger(&context)?,
    };
    context.general = Some(general);
    Ok(context)
}

/// Builds the send options: slugs first (REQ-SAG-0004), then config, model, provider and the
/// loggers (the callables, else the defaults).
fn send_options(
    model: &str,
    project: &str,
    agent_role: &str,
    provider: Option<&str>,
    general_log: Option<Py<PyAny>>,
    session_log: Option<Py<PyAny>>,
    config_dir: Option<PathBuf>,
) -> Result<SendOptions, DecideError> {
    let project = Slug::new("project", project)?;
    let agent_role = Slug::new("agent_role", agent_role)?;
    let mut probe = Context::new(SERVICE);
    probe.config_dir = config_dir.clone();
    let config = load_config(&probe)?;
    let model = ModelSelector::parse(model, &config)?;
    let provider = provider.map(str::parse::<Provider>).transpose()?;
    let context = context(general_log, config_dir)?;
    let session = match session_log {
        Some(callable) => Some(SessionLogger::injected(
            callable_logger(callable, SessionLogger::SERVICE)?,
            session_identity(&project, &agent_role)?,
        )),
        None => None,
    };
    let mut options = SendOptions {
        model,
        provider,
        project,
        agent_role,
        context,
        session,
    };
    if options.session.is_none() {
        options.session = Some(session_logger(&options)?);
    }
    Ok(options)
}

/// Closes the loggers of an operation on the worker thread, after the operation, so every
/// record and row is written (or with its callable) before the call returns.
///
/// A callable that raised made its writes refused writes, which `close` reports as lost rows;
/// they are returned as warnings and never change the result (REQ-SAG-0008).
fn close_loggers(
    general: Option<GeneralLogger>,
    session: Option<SessionLogger>,
) -> Vec<DecideError> {
    let mut failed = Vec::new();
    if let Some(Err(error)) = session.map(SessionLogger::close) {
        failed.push(DecideError::from(error));
    }
    if let Some(Err(error)) = general.map(GeneralLogger::close) {
        failed.push(DecideError::from(error));
    }
    failed
}

fn token(cancel: Option<&CancelToken>) -> sc_agent_types::CancelToken {
    cancel.map_or_else(sc_agent_types::CancelToken::new, |token| token.0.clone())
}

/// `decide send`: one request, always `Decision::decide`; returns its result line (JSON).
///
/// `images` maps part names to PNG or JPEG bytes. Raises `SagError` for a command error; a
/// failed decision is data in the returned line.
#[pyfunction]
#[pyo3(signature = (request, *, model, project, agent_role, provider = None, images = None, long_context = false, cancel = None, general_log = None, session_log = None, config_dir = None))]
#[expect(
    clippy::too_many_arguments,
    reason = "one keyword argument per decide send flag (ANC-006)"
)]
fn send(
    py: Python<'_>,
    request: &str,
    model: &str,
    project: &str,
    agent_role: &str,
    provider: Option<&str>,
    images: Option<BTreeMap<String, Vec<u8>>>,
    long_context: bool,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
    session_log: Option<Py<PyAny>>,
    config_dir: Option<PathBuf>,
) -> PyResult<String> {
    // The request is checked before any logger opens, so a refused one writes no file.
    let images = images.unwrap_or_default().into_iter().collect();
    let request = build_request_bytes(request.as_bytes(), images, long_context, None)
        .map_err(|error| sag_error(&error))?;
    let options = send_options(
        model,
        project,
        agent_role,
        provider,
        general_log,
        session_log,
        config_dir,
    )
    .map_err(|error| sag_error(&error))?;
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let (line, _) = run_interruptible(py, &cancel, None, move |_| {
        let mut options = options;
        let mut line = None;
        let sent = decide::send(&options, request, &worker_cancel, &mut |result| {
            line = Some(decide::output::result_text(&result)?);
            Ok(())
        });
        let (general, session) = (options.context.general.take(), options.session.take());
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
                Err(DecideError::from(
                    sc_decision_schema::DecisionError::Shutdown {
                        reason: "decide::send returned without a result".to_owned(),
                    },
                )),
                closed,
            ),
        }
    })?;
    Ok(line)
}

/// `decide send --batch`: always `Decision::decide_stream`; returns the result lines in
/// finish order and passes each to `on_result` as it arrives.
#[pyfunction]
#[pyo3(signature = (requests, *, model, project, agent_role, provider = None, cancel = None, on_result = None, general_log = None, session_log = None, config_dir = None))]
#[expect(
    clippy::too_many_arguments,
    reason = "one keyword argument per decide send flag (ANC-006)"
)]
fn send_batch(
    py: Python<'_>,
    requests: Vec<BatchItem>,
    model: &str,
    project: &str,
    agent_role: &str,
    provider: Option<&str>,
    cancel: Option<&CancelToken>,
    on_result: Option<Py<PyAny>>,
    general_log: Option<Py<PyAny>>,
    session_log: Option<Py<PyAny>>,
    config_dir: Option<PathBuf>,
) -> PyResult<Vec<String>> {
    let requests = build_batch(requests.into_iter().map(BatchInput::from).collect())
        .map_err(|error| sag_error(&error))?;
    let options = send_options(
        model,
        project,
        agent_role,
        provider,
        general_log,
        session_log,
        config_dir,
    )
    .map_err(|error| sag_error(&error))?;
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let ((), lines) = run_interruptible(py, &cancel, on_result.as_ref(), move |forward| {
        let mut options = options;
        let outcome = decide::send_batch(&options, requests, &worker_cancel, forward);
        let (general, session) = (options.context.general.take(), options.session.take());
        drop(options);
        (outcome, close_loggers(general, session))
    })?;
    Ok(lines)
}

/// `decide models`: the TypeSafe models list, or a configured runner's (JSON).
#[pyfunction]
#[pyo3(signature = (runner = None, cancel = None, general_log = None, config_dir = None))]
fn models(
    py: Python<'_>,
    runner: Option<&str>,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
    config_dir: Option<PathBuf>,
) -> PyResult<String> {
    let runner = runner
        .map(|runner| runner_name(runner, config_dir.clone()))
        .transpose()
        .map_err(|error| sag_error(&error))?;
    let options = ModelsOptions {
        runner,
        context: context(general_log, config_dir).map_err(|error| sag_error(&error))?,
    };
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let (text, _) = run_interruptible(py, &cancel, None, move |_| {
        let mut options = options;
        let list = decide::models(&options, &worker_cancel);
        let general = options.context.general.take();
        drop(options);
        let closed = close_loggers(general, None);
        (list.and_then(|list| decide::output::to_json(&list)), closed)
    })?;
    Ok(text)
}

/// `runner` as a `RunnerName`, parsed once at the edge; a name that cannot be a runner name
/// is no configured runner either (`decide.unknown_runner`).
fn runner_name(runner: &str, config_dir: Option<PathBuf>) -> Result<RunnerName, DecideError> {
    RunnerName::try_from(runner.to_owned()).or_else(|_| {
        let mut probe = Context::new(SERVICE);
        probe.config_dir = config_dir;
        let config = load_config(&probe)?;
        Err(DecideError::UnknownRunner {
            name: runner.to_owned(),
            known: config.local_runners.keys().cloned().collect(),
        })
    })
}

/// `decide doctor`: detected keys, missing provider models and config ties (JSON).
#[pyfunction]
#[pyo3(signature = (cancel = None, general_log = None, config_dir = None))]
fn doctor(
    py: Python<'_>,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
    config_dir: Option<PathBuf>,
) -> PyResult<String> {
    let options = DoctorOptions {
        context: context(general_log, config_dir).map_err(|error| sag_error(&error))?,
    };
    let cancel = token(cancel);
    let worker_cancel = cancel.clone();
    let (text, _) = run_interruptible(py, &cancel, None, move |_| {
        let mut options = options;
        let report = decide::doctor(&options, &worker_cancel);
        let general = options.context.general.take();
        drop(options);
        let closed = close_loggers(general, None);
        (
            report.and_then(|report| decide::output::to_json(&report)),
            closed,
        )
    })?;
    Ok(text)
}

/// The `sc_agent` module: `send`, `send_batch`, `models`, `doctor`, `CancelToken`, `SagError`.
#[pymodule]
fn sc_agent(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(send, module)?)?;
    module.add_function(wrap_pyfunction!(send_batch, module)?)?;
    module.add_function(wrap_pyfunction!(models, module)?)?;
    module.add_function(wrap_pyfunction!(doctor, module)?)?;
    module.add_class::<CancelToken>()?;
    module.add("SagError", module.py().get_type::<SagError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC: REQ-SAG-0007. The string returned for a result keeps 0.123456789 unchanged.
    #[test]
    fn a_probability_is_returned_unchanged() {
        let body = br#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.123456789}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
        let response =
            sc_decision_schema::SystemOneResponse::from_json(200, body).expect("response");
        let request = build_request_bytes(
            br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#,
            Vec::new(),
            false,
            None,
        )
        .expect("request");
        let result = DecisionResult::new(
            sc_agent_types::SessionId::new(),
            0,
            request.digests().clone(),
            Ok(response),
        );
        let text = decide::output::result_text(&result).expect("string");
        assert!(text.contains("\"noul\":0.123456789"), "{text}");
    }
}
