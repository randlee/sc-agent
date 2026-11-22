use pyo3::{
    prelude::*,
    types::{PyCapsule, PyDict},
};
use sc_agent::{Error, error};
use sc_observability::v2::Logger;
use serde_json::{Value, json};
use std::{
    ffi::CStr,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};

fn invalid(value: impl ToString) -> Error {
    error("sc_agent_py.invalid_option", value)
}
fn decode(value: &Bound<'_, PyAny>) -> Result<Value, Error> {
    let encoded = value
        .py()
        .import("json")
        .and_then(|m| m.call_method1("dumps", (value,)))
        .and_then(|v| v.extract::<String>())
        .map_err(invalid)?;
    serde_json::from_str(&encoded).map_err(invalid)
}
fn raw(py: Python<'_>, value: Value) -> Py<PyAny> {
    match py
        .import("json")
        .and_then(|m| m.call_method1("loads", (value.to_string(),)))
    {
        Ok(value) => value.unbind(),
        Err(_) => py.None(),
    }
}
fn required<'a, 'py>(value: Option<&'a Bound<'py, PyAny>>) -> Result<&'a Bound<'py, PyAny>, Error> {
    value
        .filter(|v| !v.is_none())
        .ok_or_else(|| invalid("value is required"))
}
fn text(value: Option<&Bound<'_, PyAny>>) -> Result<String, Error> {
    required(value)?.extract().map_err(invalid)
}
fn handle<T: Send + Sync + 'static>(
    value: &Bound<'_, PyAny>,
    name: &CStr,
) -> Result<Arc<T>, Error> {
    let pointer = value
        .cast::<PyCapsule>()
        .map_err(invalid)?
        .pointer_checked(Some(name))
        .map_err(invalid)?
        .cast::<Arc<T>>();
    // The checked capsule is created in this extension with exactly Arc<T>.
    Ok(unsafe { pointer.as_ref() }.clone())
}
fn capsule<T: Send + Sync + 'static>(
    py: Python<'_>,
    value: T,
    name: &'static CStr,
) -> Result<Py<PyAny>, Error> {
    let result = PyDict::new(py);
    result.set_item("ok", true).map_err(invalid)?;
    result
        .set_item(
            "value",
            PyCapsule::new_with_value(py, Arc::new(value), name).map_err(invalid)?,
        )
        .map_err(invalid)?;
    Ok(result.into_any().unbind())
}
fn respond(
    py: Python<'_>,
    given: Option<&Bound<'_, PyAny>>,
    name: &str,
    run: impl FnOnce(&Logger) -> Result<Value, Error>,
) -> Py<PyAny> {
    let owned = given.is_none_or(|v| v.is_none());
    let logger = match given.filter(|v| !v.is_none()) {
        Some(value) => handle::<Logger>(value, c"sc_agent.Logger"),
        None => sc_agent::default_logger(name).map(Arc::new),
    };
    match logger {
        Err(error) => raw(py, sc_agent::result(Err(error))),
        Ok(logger) => {
            let value = sc_agent::report(&logger, run(&logger));
            let _ = logger.flush();
            if owned {
                let _ = logger.shutdown();
            }
            raw(py, value)
        }
    }
}
fn option<T: for<'a, 'py> FromPyObject<'a, 'py>>(
    named: &[(&str, Option<&Bound<'_, PyAny>>)],
    key: &str,
) -> Result<Option<T>, Error> {
    named
        .iter()
        .find(|(name, _)| *name == key)
        .and_then(|(_, v)| *v)
        .filter(|v| !v.is_none())
        .map(|v| {
            v.extract::<T>()
                .map_err(|_| invalid(format!("invalid {key}")))
        })
        .transpose()
}
fn options(named: &[(&str, Option<&Bound<'_, PyAny>>)]) -> Result<sc_agent::Options, Error> {
    let timeout =
        Duration::try_from_secs_f64(option::<f64>(named, "timeout_secs")?.unwrap_or(120.0))
            .map_err(|e| error("decidr.invalid_request_timeout", e))?;
    let workers = option::<usize>(named, "workers")?;
    let agent_role =
        option::<String>(named, "agent_role")?.ok_or_else(|| invalid("agent_role is required"))?;
    let model = option::<String>(named, "model")?.unwrap_or_else(|| "jev".into());
    let provider = option::<String>(named, "provider")?;
    sc_agent::model(&model, provider.as_deref())?;
    let (root, repo) = sc_agent::repository()?;
    let global = option::<String>(named, "config_dir")?
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|p| p.join(".sc-agent")))
        .ok_or_else(|| error("config.home", "Home directory unavailable"))?;
    let project = option::<String>(named, "project")?;
    let mut settings = sc_agent::config(
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
    Ok(sc_agent::Options {
        settings,
        model,
        provider,
        agent_role,
        timeout,
        base_url: option(named, "base_url")?,
        root,
        repo,
        log_root: sc_agent::log_root("decidr")?,
    })
}
fn request(value: Option<&Bound<'_, PyAny>>) -> Result<Value, Error> {
    let value = required(value)?;
    let parsed = if let Ok(text) = value.extract::<String>() {
        serde_json::from_str(&text).map_err(|e| error("decision.request_parse", e))?
    } else {
        decode(value)?
    };
    if !parsed.is_object() {
        return Err(error(
            "decision.request_parse",
            "request must be a JSON object",
        ));
    }
    Ok(parsed)
}
fn cancel_flag(value: Option<&Bound<'_, PyAny>>) -> Result<Arc<sc_agent::Cancel>, Error> {
    value
        .filter(|v| !v.is_none())
        .map(|v| {
            v.getattr("_flag")
                .map_err(invalid)
                .and_then(|v| handle::<sc_agent::Cancel>(&v, c"sc_agent.Cancel"))
        })
        .transpose()
        .map(|v| v.unwrap_or_else(|| Arc::new(sc_agent::Cancel::default())))
}
fn callback(value: Option<&Bound<'_, PyAny>>) -> Result<Option<Py<PyAny>>, Error> {
    value
        .filter(|v| !v.is_none())
        .map(|v| {
            if v.is_callable() {
                Ok(v.clone().unbind())
            } else {
                Err(invalid("callback must be callable"))
            }
        })
        .transpose()
}
fn wait(delay: Duration, callback: &Option<Py<PyAny>>, cancel: &sc_agent::Cancel) {
    if let Some(callback) = callback {
        Python::attach(|py| {
            if let Err(error) = callback.call1(py, (delay.as_secs_f64(),))
                && error.is_instance_of::<pyo3::exceptions::PyKeyboardInterrupt>(py)
            {
                cancel.cancel();
            }
        });
    } else {
        cancel.wait(delay);
    }
}
fn interruptible(
    py: Python<'_>,
    cancel: &sc_agent::Cancel,
    run: impl FnOnce() -> Result<Value, Error> + Send,
) -> Result<Value, Error> {
    std::thread::scope(|scope| {
        let (sender, mut receiver) = mpsc::sync_channel(1);
        scope.spawn(move || {
            let _ = sender.send(run());
        });
        loop {
            if let Err(error) = py.check_signals()
                && error.is_instance_of::<pyo3::exceptions::PyKeyboardInterrupt>(py)
            {
                cancel.cancel();
            }
            let receive = &mut receiver;
            match py.detach(move || receive.recv_timeout(Duration::from_millis(20))) {
                Ok(value) => return value,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => return Err(invalid(error)),
            }
        }
    })
}

fn lookup(
    name: &str,
    callback: &Option<Py<PyAny>>,
    cancel: &sc_agent::Cancel,
) -> Result<String, String> {
    match callback {
        None => sc_agent::os_keyring(name),
        Some(callback) => Python::attach(|py| {
            callback
                .call1(py, (name,))
                .and_then(|v| v.extract::<String>(py))
                .map_err(|e| {
                    if e.is_instance_of::<pyo3::exceptions::PyKeyboardInterrupt>(py) {
                        cancel.cancel();
                    }
                    e.to_string()
                })
        }),
    }
}
#[pyfunction(signature=(name=None))]
fn logger(py: Python<'_>, name: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    match text(name)
        .and_then(|name| sc_agent::default_logger(&name))
        .and_then(|logger| capsule(py, logger, c"sc_agent.Logger"))
    {
        Ok(value) => value,
        Err(error) => respond(py, None, "decidr", |_| Err(error)),
    }
}
#[pyfunction(signature=(logger=None,event=None))]
fn log(
    py: Python<'_>,
    logger: Option<&Bound<'_, PyAny>>,
    event: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, logger, "decidr", |logger| {
        let supplied = decode(required(event)?)?;
        let mut event = json!({"version":"v1", "timestamp":sc_observability::Timestamp::now_utc(),
                              "service":logger.service_name(), "identity":sc_observability::ProcessIdentity::default(),
                              "target":"sc-agent", "level":sc_observability::Level::Info, "fields":{}});
        if let (Some(event), Some(supplied)) = (event.as_object_mut(), supplied.as_object()) {
            event.extend(supplied.clone());
        } else {
            return Err(invalid("event must be an object"));
        }
        let event = serde_json::from_value(event).map_err(invalid)?;
        let _ = logger.log(event);
        Ok(Value::Null)
    })
}
#[pyfunction(signature=(logger=None))]
fn flush(py: Python<'_>, logger: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    match required(logger).and_then(|v| handle::<Logger>(v, c"sc_agent.Logger")) {
        Ok(logger) => {
            let _ = logger.flush();
            raw(py, sc_agent::result(Ok(Value::Null)))
        }
        Err(error) => respond(py, None, "decidr", |_| Err(error)),
    }
}
#[pyfunction(signature=(request=None,question=None,*,general_log=None))]
fn digest(
    py: Python<'_>,
    request: Option<&Bound<'_, PyAny>>,
    question: Option<&Bound<'_, PyAny>>,
    general_log: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, general_log, "decidr", |_| {
        sc_agent::digest(&decode(required(request)?)?, &text(question)?).map(Value::String)
    })
}
#[expect(
    clippy::too_many_arguments,
    reason = "PyO3 requires a parameter for each public keyword"
)]
#[pyfunction(signature=(request=None,*,model=None,provider=None,project=None,agent_role=None,config_dir=None,general_log=None,timeout_secs=None,cancel=None,base_url=None,sleep=None,keyring_lookup=None))]
fn send(
    py: Python<'_>,
    request: Option<&Bound<'_, PyAny>>,
    model: Option<&Bound<'_, PyAny>>,
    provider: Option<&Bound<'_, PyAny>>,
    project: Option<&Bound<'_, PyAny>>,
    agent_role: Option<&Bound<'_, PyAny>>,
    config_dir: Option<&Bound<'_, PyAny>>,
    general_log: Option<&Bound<'_, PyAny>>,
    timeout_secs: Option<&Bound<'_, PyAny>>,
    cancel: Option<&Bound<'_, PyAny>>,
    base_url: Option<&Bound<'_, PyAny>>,
    sleep: Option<&Bound<'_, PyAny>>,
    keyring_lookup: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, general_log, "decidr", |logger| {
        let value = self::request(request)?;
        let options = options(&[
            ("model", model),
            ("provider", provider),
            ("project", project),
            ("agent_role", agent_role),
            ("config_dir", config_dir),
            ("timeout_secs", timeout_secs),
            ("base_url", base_url),
        ])?;
        let flag = cancel_flag(cancel)?;
        let sleep = callback(sleep)?;
        let keyring = callback(keyring_lookup)?;
        interruptible(py, &flag, || {
            sc_agent::send(
                &value,
                &options,
                logger,
                &flag,
                &|delay| wait(delay, &sleep, &flag),
                &|name| lookup(name, &keyring, &flag),
            )
        })
    })
}
#[expect(
    clippy::too_many_arguments,
    reason = "PyO3 requires a parameter for each public keyword"
)]
#[pyfunction(signature=(requests=None,*,model=None,provider=None,project=None,agent_role=None,config_dir=None,general_log=None,timeout_secs=None,workers=None,on_result=None,cancel=None,base_url=None,sleep=None,keyring_lookup=None))]
fn send_batch(
    py: Python<'_>,
    requests: Option<&Bound<'_, PyAny>>,
    model: Option<&Bound<'_, PyAny>>,
    provider: Option<&Bound<'_, PyAny>>,
    project: Option<&Bound<'_, PyAny>>,
    agent_role: Option<&Bound<'_, PyAny>>,
    config_dir: Option<&Bound<'_, PyAny>>,
    general_log: Option<&Bound<'_, PyAny>>,
    timeout_secs: Option<&Bound<'_, PyAny>>,
    workers: Option<&Bound<'_, PyAny>>,
    on_result: Option<&Bound<'_, PyAny>>,
    cancel: Option<&Bound<'_, PyAny>>,
    base_url: Option<&Bound<'_, PyAny>>,
    sleep: Option<&Bound<'_, PyAny>>,
    keyring_lookup: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, general_log, "decidr", |logger| {
        let requests = required(requests)?
            .try_iter()
            .map_err(invalid)?
            .map(|v| v.map_err(invalid).and_then(|v| request(Some(&v))))
            .collect::<Result<Vec<_>, _>>()?;
        let options = options(&[
            ("model", model),
            ("provider", provider),
            ("project", project),
            ("agent_role", agent_role),
            ("config_dir", config_dir),
            ("timeout_secs", timeout_secs),
            ("base_url", base_url),
            ("workers", workers),
        ])?;
        let flag = cancel_flag(cancel)?;
        let sleep = callback(sleep)?;
        let keyring = callback(keyring_lookup)?;
        let completed = callback(on_result)?;
        let on_result = |index, value: &Value| {
            if let Some(callback) = &completed {
                Python::attach(|py| {
                    if let Err(error) = callback.call1(py, (index, raw(py, value.clone())))
                        && error.is_instance_of::<pyo3::exceptions::PyKeyboardInterrupt>(py)
                    {
                        flag.cancel();
                    }
                });
            }
        };
        interruptible(py, &flag, || {
            sc_agent::batch(
                &requests,
                &options,
                logger,
                &flag,
                &|delay| wait(delay, &sleep, &flag),
                &|name| lookup(name, &keyring, &flag),
                &on_result,
            )
        })
    })
}
#[pyfunction]
fn _new_cancel(py: Python<'_>) -> Py<PyAny> {
    match capsule(py, sc_agent::Cancel::default(), c"sc_agent.Cancel") {
        Ok(value) => value,
        Err(error) => raw(py, sc_agent::result(Err(error))),
    }
}
#[pyfunction(signature=(flag=None))]
fn _cancel(py: Python<'_>, flag: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    raw(
        py,
        sc_agent::result(
            required(flag)
                .and_then(|v| handle::<sc_agent::Cancel>(v, c"sc_agent.Cancel"))
                .map(|flag| {
                    flag.cancel();
                    Value::Null
                }),
        ),
    )
}
#[pyfunction(signature=(*,general_log=None))]
fn languages(py: Python<'_>, general_log: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    respond(py, general_log, "sitr", |logger| {
        sc_agent::log(logger, "sitr.languages", json!({}));
        Ok(sc_treesitter::languages())
    })
}
#[pyfunction(signature=(language=None,*,general_log=None))]
fn queries(
    py: Python<'_>,
    language: Option<&Bound<'_, PyAny>>,
    general_log: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, general_log, "sitr", |logger| {
        let value = sc_treesitter::queries(&text(language)?)?;
        sc_agent::log(logger, "sitr.queries", json!({}));
        Ok(value)
    })
}
#[pyfunction(signature=(language=None,source=None,query=None,*,general_log=None))]
fn query(
    py: Python<'_>,
    language: Option<&Bound<'_, PyAny>>,
    source: Option<&Bound<'_, PyAny>>,
    query: Option<&Bound<'_, PyAny>>,
    general_log: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, general_log, "sitr", |logger| {
        let value = sc_treesitter::query(
            &sc_treesitter::language(&text(language)?)?,
            &text(source)?,
            &text(query)?,
        )?;
        sc_agent::log(logger, "sitr.query", json!({}));
        Ok(value)
    })
}
#[pyfunction(signature=(language=None,name=None,source=None,*,general_log=None))]
fn run(
    py: Python<'_>,
    language: Option<&Bound<'_, PyAny>>,
    name: Option<&Bound<'_, PyAny>>,
    source: Option<&Bound<'_, PyAny>>,
    general_log: Option<&Bound<'_, PyAny>>,
) -> Py<PyAny> {
    respond(py, general_log, "sitr", |logger| {
        let value = sc_treesitter::predefined(&text(language)?, &text(name)?, &text(source)?)?;
        sc_agent::log(logger, "sitr.run", json!({}));
        Ok(value)
    })
}
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(logger, module)?)?;
    module.add_function(wrap_pyfunction!(log, module)?)?;
    module.add_function(wrap_pyfunction!(flush, module)?)?;
    module.add_function(wrap_pyfunction!(digest, module)?)?;
    module.add_function(wrap_pyfunction!(send, module)?)?;
    module.add_function(wrap_pyfunction!(send_batch, module)?)?;
    module.add_function(wrap_pyfunction!(_new_cancel, module)?)?;
    module.add_function(wrap_pyfunction!(_cancel, module)?)?;
    module.add_function(wrap_pyfunction!(languages, module)?)?;
    module.add_function(wrap_pyfunction!(queries, module)?)?;
    module.add_function(wrap_pyfunction!(query, module)?)?;
    module.add_function(wrap_pyfunction!(run, module)?)?;
    Ok(())
}
