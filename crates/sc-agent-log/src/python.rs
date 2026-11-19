//! Direct delivery to caller-owned Python targets.
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sc_agent_types::ErrorDetail;
use sc_observability::v2::LogEvent;

use crate::LogError;

pub(crate) enum PythonTarget {
    Logger(Py<PyAny>),
    Callable(Py<PyAny>),
}

impl PythonTarget {
    pub(crate) fn classify(target: Py<PyAny>) -> Result<Self, LogError> {
        Python::attach(|py| {
            let value = target.bind(py);
            if value.is_callable() {
                return Ok(Self::Callable(target));
            }
            let obs = py.import("sc_observability").map_err(classify_refused)?;
            if value
                .is_instance(&obs.getattr("Logger").map_err(classify_refused)?)
                .map_err(classify_refused)?
                || value
                    .is_instance(&obs.getattr("AttachedLogger").map_err(classify_refused)?)
                    .map_err(classify_refused)?
            {
                Ok(Self::Logger(target))
            } else {
                Err(LogError::InvalidTarget)
            }
        })
    }

    pub(crate) fn admit(&self, event: &LogEvent) -> Result<(), LogError> {
        Python::attach(|py| -> Result<(), LogError> {
            let json = py.import("json").map_err(refused)?;
            let level = match event.level {
                sc_observability::Level::Trace => "trace",
                sc_observability::Level::Debug => "debug",
                sc_observability::Level::Info => "info",
                sc_observability::Level::Warn => "warn",
                sc_observability::Level::Error => "error",
            };
            // sc-observability is pinned to 1.5.0. When upgrading it, re-check
            // this mapping against every LogEvent field on both Rust and Python sides.
            let payload = serde_json::json!({"level":level, "target":event.target, "action":event.action,
                "message":event.message, "request_id":event.request_id, "correlation_id":event.correlation_id,
                "outcome":event.outcome, "trace":event.trace, "fields":event.fields});
            let record = json
                .call_method1("loads", (payload.to_string(),))
                .map_err(refused)?;
            match self {
                Self::Callable(target) => {
                    target.call1(py, (record,)).map_err(refused)?;
                    Ok(())
                }
                Self::Logger(target) => {
                    let obs = py.import("sc_observability").map_err(refused)?;
                    let kwargs = record
                        .cast::<PyDict>()
                        .map_err(|error| refused(error.into()))?;
                    let typed = obs
                        .getattr("LogEvent")
                        .and_then(|class| class.call((), Some(kwargs)))
                        .map_err(refused)?;
                    let result = target.call_method1(py, "log", (typed,)).map_err(refused)?;
                    let bound = result.bind(py);
                    if bound
                        .is_instance(&obs.getattr("Ok").map_err(refused)?)
                        .map_err(refused)?
                    {
                        Ok(())
                    } else if bound
                        .is_instance(&obs.getattr("Err").map_err(refused)?)
                        .map_err(refused)?
                    {
                        let error = result.getattr(py, "error").map_err(refused)?;
                        let code: String = error
                            .getattr(py, "code")
                            .and_then(|v| v.extract(py))
                            .map_err(refused)?;
                        let message: String = error
                            .getattr(py, "message")
                            .and_then(|v| v.extract(py))
                            .map_err(refused)?;
                        Err(LogError::PythonRejected {
                            cause: Box::new(ErrorDetail::new(code, message, None)),
                        })
                    } else {
                        Err(failure(
                            "python.invalid_result",
                            "Logger.log must return Ok or Err",
                        ))
                    }
                }
            }
        })
    }
}

fn failure(code: &str, message: impl Into<String>) -> LogError {
    LogError::PythonRejected {
        cause: Box::new(ErrorDetail::new(code, message.into(), None)),
    }
}
fn refused(error: PyErr) -> LogError {
    failure("python.exception", error.to_string())
}

fn classify_refused(error: PyErr) -> LogError {
    LogError::PythonUnavailable {
        cause: Box::new(ErrorDetail::new(
            "python.exception",
            error.to_string(),
            None,
        )),
    }
}
