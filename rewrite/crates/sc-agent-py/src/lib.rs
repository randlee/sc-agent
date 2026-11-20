use pyo3::{prelude::*, types::PyDict};
use serde_json::Value;

fn as_python<'py>(py: Python<'py>, value: &Value) -> Bound<'py, PyAny> {
    py.import("json")
        .and_then(|m| m.call_method1("loads", (value.to_string(),)))
        .unwrap_or_else(|_| py.None().into_bound(py))
}

#[pyclass(module = "sc_agent")]
struct Handle {
    logger: sc_observability::v2::Logger,
}

#[pymethods]
impl Handle {
    fn __call__<'py>(&self, py: Python<'py>, record: &Bound<'py, PyAny>) -> Bound<'py, PyAny> {
        let action = record
            .get_item("action")
            .and_then(|x| x.extract::<String>())
            .unwrap_or_default();
        let result = match ::sc_agent::log(&self.logger, &action) {
            Ok(()) => serde_json::json!({"ok": true, "value": null}),
            Err(e) => e,
        };
        as_python(py, &result)
    }
}

#[pyfunction]
fn logger<'py>(py: Python<'py>, name: &Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    let name = name.extract::<String>().unwrap_or_default();
    match ::sc_agent::open_default_logger(&name, "DECIDR_LOG_ROOT") {
        Ok(log) => {
            let result = PyDict::new(py);
            let _ = result.set_item("ok", true);
            if let Ok(handle) = Py::new(py, Handle { logger: log }) {
                let _ = result.set_item("value", handle);
                result.into_any()
            } else {
                as_python(
                    py,
                    &::sc_agent::error("log_open", "Cannot create logger handle", "Check logger"),
                )
            }
        }
        Err(e) => as_python(py, &e),
    }
}

fn record(_py: Python<'_>, handle: Option<&Bound<'_, PyAny>>, action: &str) -> Result<(), Value> {
    if let Some(handle) = handle {
        let Ok(handle) = handle.extract::<PyRef<'_, Handle>>() else {
            return Err(::sc_agent::error(
                "invalid_logger",
                "Logger must be a sc-decidr handle",
                "Open a logger by client name",
            ));
        };
        ::sc_agent::log(&handle.logger, action)
    } else {
        ::sc_agent::open_default_logger("decidr", "DECIDR_LOG_ROOT")
            .and_then(|log| ::sc_agent::log(&log, action))
    }
}

#[pyfunction(signature = (input, model, agent_role, provider=None, project=None, general_log=None))]
fn send<'py>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    agent_role: &Bound<'py, PyAny>,
    provider: Option<&Bound<'py, PyAny>>,
    project: Option<&Bound<'py, PyAny>>,
    general_log: Option<&Bound<'py, PyAny>>,
) -> Bound<'py, PyAny> {
    let (Ok(input), Ok(model), Ok(agent_role)) = (
        input.extract::<String>(),
        model.extract::<String>(),
        agent_role.extract::<String>(),
    ) else {
        return as_python(
            py,
            &::sc_agent::error(
                "invalid_request",
                "Expected string arguments",
                "Check the request and options",
            ),
        );
    };
    let provider = provider.and_then(|p| p.extract::<String>().ok());
    let project = project.and_then(|p| p.extract::<String>().ok());
    if let Err(e) = record(py, general_log, "decidr.send") {
        return as_python(py, &e);
    }
    let result = py.detach(move || {
        ::sc_agent::send(
            &input,
            &model,
            provider.as_deref(),
            project.as_deref(),
            Some(&agent_role),
            &std::thread::sleep,
            None,
        )
    });
    as_python(py, &result)
}

#[pyfunction(signature = (inputs, model, agent_role, provider=None, project=None, workers=None, general_log=None))]
fn send_batch<'py>(
    py: Python<'py>,
    inputs: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    agent_role: &Bound<'py, PyAny>,
    provider: Option<&Bound<'py, PyAny>>,
    project: Option<&Bound<'py, PyAny>>,
    workers: Option<&Bound<'py, PyAny>>,
    general_log: Option<&Bound<'py, PyAny>>,
) -> Bound<'py, PyAny> {
    let (Ok(inputs), Ok(model), Ok(agent_role)) = (
        inputs.extract::<Vec<String>>(),
        model.extract::<String>(),
        agent_role.extract::<String>(),
    ) else {
        return as_python(
            py,
            &::sc_agent::error(
                "invalid_request",
                "Invalid batch arguments",
                "Check the requests and options",
            ),
        );
    };
    let provider = provider.and_then(|p| p.extract::<String>().ok());
    let project = project.and_then(|p| p.extract::<String>().ok());
    let workers = workers.and_then(|p| p.extract::<usize>().ok()).unwrap_or(2);
    if workers == 0 {
        return as_python(
            py,
            &::sc_agent::error(
                "invalid_workers",
                "Worker count must be positive",
                "Choose at least one worker",
            ),
        );
    }
    if let Err(e) = record(py, general_log, "decidr.send_batch") {
        return as_python(py, &e);
    }
    let mut rows = Vec::new();
    let mut cancelled = false;
    for (number, chunk) in inputs.chunks(workers).enumerate() {
        if py.check_signals().is_err() {
            cancelled = true;
            break;
        }
        let chunk = chunk.to_vec();
        let model = model.clone();
        let provider = provider.clone();
        let project = project.clone();
        let agent_role = agent_role.clone();
        let result = py.detach(move || {
            ::sc_agent::send_batch(
                &chunk,
                &model,
                provider.as_deref(),
                project.as_deref(),
                Some(&agent_role),
                workers,
                &std::thread::sleep,
                None,
            )
        });
        if let Some(values) = result.get("value").and_then(Value::as_array) {
            for row in values {
                let mut row = row.clone();
                if let Some(position) = row.get("position").and_then(Value::as_u64) {
                    if let Some(object) = row.as_object_mut() {
                        object.insert(
                            "position".into(),
                            serde_json::json!(position + (number * workers) as u64),
                        );
                    }
                }
                rows.push(row);
            }
        } else {
            return as_python(py, &result);
        }
    }
    as_python(
        py,
        &serde_json::json!({"ok": true, "value": rows, "cancelled": cancelled}),
    )
}

#[pyfunction]
fn question_hash<'py>(py: Python<'py>, question: &Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    match question.extract::<String>() {
        Ok(text) => match ::sc_agent::question_hash(&text) {
            Ok(hash) => as_python(py, &serde_json::json!({"ok": true, "value": hash})),
            Err(e) => as_python(py, &e),
        },
        Err(_) => as_python(
            py,
            &::sc_agent::error(
                "invalid_question",
                "Question must be text",
                "Provide question text",
            ),
        ),
    }
}

#[pymodule]
mod _native {
    #[pymodule_export]
    use super::{logger, question_hash, send, send_batch};
}
