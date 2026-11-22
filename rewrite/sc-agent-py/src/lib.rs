use pyo3::{
    prelude::*,
    types::{PyCapsule, PyDict, PyTuple},
};
use sc_agent::{Error, error};
use sc_observability::v2::Logger;
use serde_json::{Value, json};
use std::{
    ffi::CStr,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

static DEFAULT_LOG: OnceLock<Result<Arc<Logger>, Error>> = OnceLock::new();
static PAUSE: Mutex<()> = Mutex::new(());
fn invalid(e: impl ToString) -> Error {
    error("sc_agent_py.invalid_option", e)
}
fn decode(value: &Bound<'_, PyAny>) -> Result<Value, Error> {
    let py = value.py();
    let encoded = py
        .import("json")
        .and_then(|m| m.call_method1("dumps", (value,)))
        .and_then(|v| v.extract::<String>())
        .map_err(invalid)?;
    serde_json::from_str(&encoded).map_err(invalid)
}
fn output(py: Python<'_>, result: Result<Value, Error>) -> Py<PyAny> {
    raw(py, sc_agent::result(result))
}
fn raw(py: Python<'_>, value: Value) -> Py<PyAny> {
    let text = value.to_string();
    match py
        .import("json")
        .and_then(|m| m.call_method1("loads", (text,)))
    {
        Ok(value) => value.unbind(),
        Err(_) => py.None(),
    }
}
fn handle<T: Send + Sync + 'static>(
    object: &Bound<'_, PyAny>,
    name: &CStr,
) -> Result<Arc<T>, Error> {
    let capsule = object.cast::<PyCapsule>().map_err(invalid)?;
    let pointer = capsule
        .pointer_checked(Some(name))
        .map_err(invalid)?
        .cast::<Arc<T>>();
    // This extension constructs the named capsule with exactly Arc<T> and retains
    // its Python owner until the Arc has been cloned.
    Ok(unsafe { pointer.as_ref() }.clone())
}
fn argument<'py>(args: &Bound<'py, PyTuple>, index: usize) -> Result<Bound<'py, PyAny>, Error> {
    args.get_item(index).map_err(invalid)
}
fn text(args: &Bound<'_, PyTuple>, index: usize) -> Result<String, Error> {
    argument(args, index)?.extract().map_err(invalid)
}
fn option<T: for<'a, 'py> FromPyObject<'a, 'py>>(
    kwargs: &Bound<'_, PyDict>,
    name: &str,
) -> Result<Option<T>, Error> {
    kwargs
        .get_item(name)
        .map_err(invalid)?
        .filter(|v| !v.is_none())
        .map(|v| {
            v.extract::<T>()
                .map_err(|_| invalid(format!("invalid {name}")))
        })
        .transpose()
}
fn general(kwargs: &Bound<'_, PyDict>, name: &str) -> Result<Arc<Logger>, Error> {
    if let Some(value) = kwargs
        .get_item("general_log")
        .map_err(invalid)?
        .filter(|v| !v.is_none())
    {
        return handle(&value, c"sc_agent.Logger");
    }
    DEFAULT_LOG
        .get_or_init(|| sc_agent::default_logger(name).map(Arc::new))
        .as_ref()
        .map(Arc::clone)
        .map_err(invalid)
}
fn capsule_result<T: Send + Sync + 'static>(
    py: Python<'_>,
    value: T,
    name: &'static CStr,
) -> Result<Py<PyAny>, Error> {
    let capsule = PyCapsule::new_with_value(py, Arc::new(value), name).map_err(invalid)?;
    let result = PyDict::new(py);
    result.set_item("ok", true).map_err(invalid)?;
    result.set_item("value", capsule).map_err(invalid)?;
    Ok(result.into_any().unbind())
}
fn execute(
    py: Python<'_>,
    operation: &str,
    args: &Bound<'_, PyTuple>,
    kwargs: &Bound<'_, PyDict>,
) -> Result<Py<PyAny>, Error> {
    let arity = match operation {
        "cancel_new" | "sitr.languages" => 0,
        "digest" | "log" => 2,
        "sitr.query" | "sitr.run" => 3,
        _ => 1,
    };
    if args.len() != arity {
        return Err(invalid(format!(
            "{operation} requires {arity} positional arguments"
        )));
    }
    if operation == "send" || operation == "send_batch" {
        for (key, _) in kwargs.iter() {
            let name = key.extract::<String>().map_err(invalid)?;
            if ![
                "model",
                "provider",
                "project",
                "agent_role",
                "timeout_secs",
                "workers",
                "config_dir",
                "general_log",
                "session_log",
                "cancel",
                "base_url",
                "sleep",
                "on_result",
            ]
            .contains(&name.as_str())
            {
                return Err(invalid(format!("unknown option {name}")));
            }
        }
    }
    if operation == "logger" {
        return capsule_result(
            py,
            sc_agent::default_logger(&text(args, 0)?)?,
            c"sc_agent.Logger",
        );
    }
    if operation == "cancel_new" {
        return capsule_result(py, AtomicBool::new(false), c"sc_agent.Cancel");
    }
    if operation == "cancel" {
        handle::<AtomicBool>(&argument(args, 0)?, c"sc_agent.Cancel")?
            .store(true, Ordering::SeqCst);
        return Ok(output(py, Ok(Value::Null)));
    }
    if ["category", "retryable"].contains(&operation) {
        let value = decode(&argument(args, 0)?)?;
        if !value.is_object() {
            return Err(invalid("error detail must be an object"));
        }
        let category = value
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("other");
        return Ok(output(
            py,
            Ok(if operation == "category" {
                json!(category)
            } else {
                json!(["busy", "network"].contains(&category))
            }),
        ));
    }
    if operation == "digest" {
        let request = decode(&argument(args, 0)?)?;
        return Ok(output(
            py,
            sc_agent::digest(&request, &text(args, 1)?).map(Value::String),
        ));
    }
    if ["log", "flush"].contains(&operation) {
        let logger = handle::<Logger>(&argument(args, 0)?, c"sc_agent.Logger")?;
        if operation == "log" {
            let event = decode(&argument(args, 1)?)?;
            let action = event
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("action is required"))?;
            sc_agent::log(
                &logger,
                action,
                event.get("fields").cloned().unwrap_or(json!({})),
            );
        }
        let _ = logger.flush();
        return Ok(output(py, Ok(Value::Null)));
    }
    if operation.starts_with("sitr.") {
        let logger = general(kwargs, "sitr")?;
        let result = match operation {
            "sitr.languages" => Ok(sc_treesitter::languages()),
            "sitr.queries" => {
                sc_treesitter::queries(&text(args, 0)?).map_err(|e| error(e.id, e.message))
            }
            "sitr.query" => sc_treesitter::query(
                &sc_treesitter::language(&text(args, 0)?).map_err(|e| error(e.id, e.message))?,
                &text(args, 1)?,
                &text(args, 2)?,
            )
            .map_err(|e| error(e.id, e.message)),
            "sitr.run" => {
                sc_treesitter::predefined(&text(args, 0)?, &text(args, 1)?, &text(args, 2)?)
                    .map_err(|e| error(e.id, e.message))
            }
            _ => Err(invalid("unknown sitr operation")),
        };
        sc_agent::log(
            &logger,
            "sitr.result",
            match &result {
                Ok(v) => json!({"value":v}),
                Err(e) => e.detail(),
            },
        );
        let _ = logger.flush();
        return Ok(output(py, result));
    }
    if argument(args, 0)?.is_none() {
        return Err(invalid("request must not be None"));
    }
    let timeout = option::<f64>(kwargs, "timeout_secs")?.unwrap_or(120.0);
    let workers = option::<i64>(kwargs, "workers")?;
    let (timeout, workers) = sc_agent::limits(timeout, workers, operation == "send_batch")?;
    for name in ["sleep", "on_result"] {
        if let Some(value) = kwargs
            .get_item(name)
            .map_err(invalid)?
            .filter(|v| !v.is_none())
            && !value.is_callable()
        {
            return Err(invalid(format!("{name} must be callable")));
        }
    }
    let model = option::<String>(kwargs, "model")?.unwrap_or_else(|| "jev".into());
    let provider = option::<String>(kwargs, "provider")?;
    sc_agent::model(&model, provider.as_deref())?;
    let (root, repo) = sc_agent::repository()?;
    let global = option::<String>(kwargs, "config_dir")?
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| sc_agent::home().map(|p| p.join(".sc-agent")))?;
    let project = option::<String>(kwargs, "project")?;
    let mut settings = sc_agent::config(
        "decidr",
        root.join(".git")
            .exists()
            .then_some(root.join(".sc/sc-agent"))
            .as_deref(),
        &global,
        &std::env::vars_os().collect::<Vec<_>>(),
        project.as_deref(),
    )?
    .config;
    if let Some(workers) = workers {
        settings.workers = workers;
    }
    let options = sc_agent::Options {
        settings,
        model,
        provider,
        agent_role: option::<String>(kwargs, "agent_role")?
            .ok_or_else(|| invalid("agent_role is required"))?,
        timeout,
        base_url: option(kwargs, "base_url")?,
        root,
        log_root: sc_agent::log_root("decidr")?,
        session_log: kwargs
            .get_item("session_log")
            .map_err(invalid)?
            .filter(|v| !v.is_none())
            .map(|v| handle::<Logger>(&v, c"sc_agent.Logger"))
            .transpose()?,
        repo,
    };
    let logger = general(kwargs, "decidr")?;
    let cancel = kwargs
        .get_item("cancel")
        .map_err(invalid)?
        .filter(|v| !v.is_none())
        .map(|v| {
            v.getattr("_flag")
                .map_err(invalid)
                .and_then(|v| handle::<AtomicBool>(&v, c"sc_agent.Cancel"))
        })
        .transpose()?
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let sleep = kwargs
        .get_item("sleep")
        .map_err(invalid)?
        .filter(|v| !v.is_none())
        .map(Bound::unbind);
    let wait = |delay: Duration| {
        if let Some(callback) = &sleep {
            Python::attach(|py| {
                let _ = callback.call1(py, (delay.as_secs_f64(),));
            });
        } else {
            let (_sender, receiver) = std::sync::mpsc::channel::<()>();
            let _ = receiver.recv_timeout(delay);
        }
    };
    let parsed = |value: Bound<'_, PyAny>| -> Result<Value, Error> {
        if let Ok(text) = value.extract::<String>() {
            serde_json::from_str(&text).map_err(|e| error("decision.request_parse", e))
        } else {
            decode(&value)
        }
    };
    let result = if operation == "send" {
        let request = parsed(argument(args, 0)?)?;
        py.detach(|| {
            sc_agent::send(
                &request,
                &options,
                &logger,
                (&cancel, &PAUSE),
                &wait,
                &sc_agent::os_keyring,
            )
        })
    } else if operation == "send_batch" {
        let requests = argument(args, 0)?
            .try_iter()
            .map_err(invalid)?
            .map(|v| v.map_err(invalid).and_then(&parsed))
            .collect::<Result<Vec<_>, _>>()?;
        let callback = kwargs
            .get_item("on_result")
            .map_err(invalid)?
            .filter(|v| !v.is_none())
            .map(Bound::unbind);
        let completed = |index: usize, value: &Value| {
            if let Some(callback) = &callback {
                Python::attach(|py| {
                    let _ = callback.call1(py, (index, raw(py, value.clone())));
                });
            }
        };
        py.detach(|| {
            sc_agent::batch(
                &requests,
                &options,
                &logger,
                (&cancel, &PAUSE),
                &wait,
                &sc_agent::os_keyring,
                &completed,
            )
        })
    } else {
        return Err(invalid("unknown operation"));
    };
    Ok(output(py, result))
}
#[pyfunction]
fn invoke(
    py: Python<'_>,
    operation: &str,
    args: &Bound<'_, PyTuple>,
    kwargs: &Bound<'_, PyDict>,
) -> Py<PyAny> {
    match execute(py, operation, args, kwargs) {
        Ok(value) => value,
        Err(error) => {
            let detail = error.detail();
            if !matches!(
                detail.get("id").and_then(Value::as_str),
                Some("decidr.invalid_request_timeout" | "decidr.invalid_workers")
            ) && let Ok(logger) = general(
                kwargs,
                if operation.starts_with("sitr.") {
                    "sitr"
                } else {
                    "decidr"
                },
            ) {
                sc_agent::log(&logger, "error", detail);
                let _ = logger.flush();
            }
            output(py, Err(error))
        }
    }
}
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(invoke, module)?)?;
    Ok(())
}
