use pyo3::{
    exceptions::PyKeyboardInterrupt,
    prelude::*,
    types::{PyCFunction, PyDict, PyTuple},
};
use sc_agent::{Error, error};
use sc_observability::v2::Logger;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
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
        // REQ-SAG-0002: a result Python cannot load comes back as an error value
        // carrying the loader's message and the whole result text, never as None.
        Err(e) => {
            let failure = sc_agent::error("sc_agent_py.result_load", format!("{e}: {value}"));
            let fields = PyDict::new(py);
            let detail = PyDict::new(py);
            let Error::Failure {
                id,
                message,
                action,
                category,
                ..
            } = failure;
            for (key, text) in [
                ("id", id),
                ("message", message.as_str()),
                ("action", action),
                ("category", category),
            ] {
                let _ = detail.set_item(key, text);
            }
            let _ = fields.set_item("ok", false);
            let _ = fields.set_item("error", detail);
            fields.into_any().unbind()
        }
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
#[pyclass(name = "Logger", frozen)]
struct PyLogger(Arc<Logger>);
fn logger_of(value: &Bound<'_, PyAny>) -> Result<Arc<Logger>, Error> {
    value
        .extract::<PyRef<'_, PyLogger>>()
        .map(|v| v.0.clone())
        .map_err(invalid)
}
fn respond(
    py: Python<'_>,
    given: Option<&Bound<'_, PyAny>>,
    name: &str,
    run: impl FnOnce(&Logger) -> Result<Value, Error>,
) -> Py<PyAny> {
    raw(py, respond_value(given, name, run))
}
fn respond_value(
    given: Option<&Bound<'_, PyAny>>,
    name: &str,
    run: impl FnOnce(&Logger) -> Result<Value, Error>,
) -> Value {
    let owned = given.is_none_or(|v| v.is_none());
    let logger = match given.filter(|v| !v.is_none()) {
        Some(value) => logger_of(value),
        None => sc_agent::default_logger(name).map(Arc::new),
    };
    match logger {
        Err(error) => sc_agent::result(Err(error)),
        Ok(logger) => {
            let value = sc_agent::report(&logger, run(&logger));
            finish(&logger, owned, value)
        }
    }
}
fn finish(logger: &Logger, owned: bool, value: Value) -> Value {
    match (
        logger.flush(),
        if owned { logger.shutdown() } else { Ok(()) },
    ) {
        (Err(e), _) => sc_agent::result(Err(error("log.flush", e))),
        (Ok(()), shutdown) => sc_agent::after_shutdown(value, shutdown),
    }
}

fn arg<T: for<'a, 'py> FromPyObject<'a, 'py>>(
    value: Option<&Bound<'_, PyAny>>,
    name: &str,
) -> Result<Option<T>, Error> {
    value
        .filter(|v| !v.is_none())
        .map(|v| {
            v.extract::<T>().map_err(|e| {
                let e: PyErr = e.into();
                invalid(format!("invalid {name}: {e}"))
            })
        })
        .transpose()
}
fn extras(extra: &Bound<'_, PyTuple>, rest: Option<&Bound<'_, PyDict>>) -> Result<(), Error> {
    if !extra.is_empty() {
        return Err(invalid("unexpected positional arguments"));
    }
    if let Some(rest) = rest
        && !rest.is_empty()
    {
        return Err(invalid(format!("unexpected arguments: {rest}")));
    }
    Ok(())
}
fn bind<'py>(
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
    positional: &[&str],
    keyword_only: &[&str],
) -> Result<Vec<Option<Bound<'py, PyAny>>>, Error> {
    if args.len() > positional.len() {
        return Err(invalid("unexpected positional arguments"));
    }
    let mut values = Vec::with_capacity(positional.len() + keyword_only.len());
    for (index, name) in positional.iter().chain(keyword_only).enumerate() {
        let positional_value = (index < positional.len() && index < args.len())
            .then(|| args.get_item(index))
            .transpose()
            .map_err(invalid)?;
        let named_value = kwargs
            .map(|kwargs| kwargs.get_item(name))
            .transpose()
            .map_err(invalid)?
            .flatten();
        if positional_value.is_some() && named_value.is_some() {
            return Err(invalid(format!("multiple values for argument '{name}'")));
        }
        values.push(positional_value.or(named_value));
    }
    if let Some(kwargs) = kwargs {
        for (key, _) in kwargs.iter() {
            let name = key.extract::<String>().map_err(invalid)?;
            if !positional.contains(&name.as_str()) && !keyword_only.contains(&name.as_str()) {
                return Err(invalid(format!("unexpected argument '{name}'")));
            }
        }
    }
    Ok(values)
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
fn callback(value: Option<&Bound<'_, PyAny>>) -> Result<Option<Py<PyAny>>, Error> {
    let Some(value) = value.filter(|v| !v.is_none()) else {
        return Ok(None);
    };
    if !value.is_callable() {
        return Err(invalid("callback must be callable"));
    }
    Ok(Some(value.clone().unbind()))
}
fn call_back<A, T>(callback: &Py<PyAny>, argument: A, cancel: &sc_agent::Cancel) -> Result<T, PyErr>
where
    A: for<'py> IntoPyObject<'py> + Send,
    T: for<'a, 'py> FromPyObject<'a, 'py>,
{
    Python::attach(|py| {
        callback
            .call1(py, (argument,))
            .and_then(|v| v.extract::<T>(py).map_err(Into::into))
            .inspect_err(|e| {
                if e.is_instance_of::<PyKeyboardInterrupt>(py) {
                    cancel.cancel();
                }
            })
    })
}
fn callback_result(
    callback_error: Mutex<Option<Error>>,
    outcome: Result<Value, Error>,
) -> Result<Value, Error> {
    if let Some(mut failure) = callback_error.into_inner().map_err(invalid)? {
        let Error::Failure { message, .. } = &mut failure;
        message.push_str(&format!(
            "; original outcome: {}",
            sc_agent::result(outcome)
        ));
        return Err(failure);
    }
    outcome
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
                && error.is_instance_of::<PyKeyboardInterrupt>(py)
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

#[pyfunction(signature=(name=None))]
fn logger(py: Python<'_>, name: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    let created = (|| -> Result<Py<PyAny>, Error> {
        let logger = sc_agent::default_logger(&text(name)?)?;
        let value = Py::new(py, PyLogger(Arc::new(logger))).map_err(invalid)?;
        let envelope = PyDict::new(py);
        envelope.set_item("ok", true).map_err(invalid)?;
        envelope.set_item("value", value).map_err(invalid)?;
        Ok(envelope.into_any().unbind())
    })();
    created.unwrap_or_else(|e| respond(py, None, "decidr", |_| Err(e)))
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
        logger.log(event).map_err(|e| error("log.write", e))?;
        Ok(Value::Null)
    })
}
#[pyfunction(signature=(logger=None))]
fn flush(py: Python<'_>, logger: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    match required(logger).and_then(|v| logger_of(v)) {
        Ok(logger) => raw(
            py,
            sc_agent::result(
                logger
                    .flush()
                    .map(|()| Value::Null)
                    .map_err(|e| error("log.flush", e)),
            ),
        ),
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
#[pyclass(name = "Run", frozen)]
struct PyRun(Result<sc_agent::Run, Value>);
impl PyRun {
    fn with(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&sc_agent::Run) -> Result<Value, Error>,
    ) -> Py<PyAny> {
        raw(
            py,
            match &self.0 {
                Ok(run) => sc_agent::report(run.logger(), f(run)),
                Err(e) => e.clone(),
            },
        )
    }
}
#[pymethods]
impl PyRun {
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "PyO3 needs a parameter per public keyword"
    )]
    #[pyo3(signature=(*extra, tool=None, model=None, project=None, agent_role=None, config_dir=None, general_log=None, timeout_secs=None, workers=None, base_url=None, sleep=None, keyring_lookup=None, **rest))]
    fn new(
        extra: &Bound<'_, PyTuple>,
        tool: Option<&Bound<'_, PyAny>>,
        model: Option<&Bound<'_, PyAny>>,
        project: Option<&Bound<'_, PyAny>>,
        agent_role: Option<&Bound<'_, PyAny>>,
        config_dir: Option<&Bound<'_, PyAny>>,
        general_log: Option<&Bound<'_, PyAny>>,
        timeout_secs: Option<&Bound<'_, PyAny>>,
        workers: Option<&Bound<'_, PyAny>>,
        base_url: Option<&Bound<'_, PyAny>>,
        sleep: Option<&Bound<'_, PyAny>>,
        keyring_lookup: Option<&Bound<'_, PyAny>>,
        rest: Option<&Bound<'_, PyDict>>,
    ) -> Self {
        let tool = match arg::<String>(tool, "tool").and_then(|v| {
            v.filter(|v| !v.is_empty())
                .ok_or_else(|| invalid("tool is required"))
        }) {
            Ok(tool) => tool,
            Err(e) => return Self(Err(sc_agent::result(Err(e)))),
        };
        let created = (|| {
            extras(extra, rest)?;
            let args = sc_agent::RunArgs {
                tool: tool.clone(),
                model: arg(model, "model")?,
                project: arg(project, "project")?,
                agent_role: arg(agent_role, "agent_role")?
                    .ok_or_else(|| invalid("agent_role is required"))?,
                config_dir: arg::<String>(config_dir, "config_dir")?.map(PathBuf::from),
                timeout_secs: arg(timeout_secs, "timeout_secs")?,
                workers: arg(workers, "workers")?,
                base_url: arg(base_url, "base_url")?,
            };
            let logger = general_log
                .filter(|v| !v.is_none())
                .map(logger_of)
                .transpose()?;
            let sleep = callback(sleep)?;
            let lookup = callback(keyring_lookup)?;
            let flag = Arc::new(sc_agent::Cancel::default());
            let mut run = sc_agent::Run::new(args, logger, flag.clone())?;
            if let Some(callback) = sleep {
                let flag = flag.clone();
                run.wait = Box::new(move |delay| {
                    Python::attach(|py| {
                        match call_back::<_, Py<PyAny>>(&callback, delay.as_secs_f64(), &flag) {
                            Ok(_) => Ok(()),
                            Err(e) if e.is_instance_of::<PyKeyboardInterrupt>(py) => Ok(()),
                            Err(e) => Err(error("sc_agent_py.callback", e)),
                        }
                    })
                });
            }
            if let Some(callback) = lookup {
                run.keyring = Box::new(move |name| {
                    call_back(&callback, name.to_owned(), &flag)
                        .map_err(|e| sc_agent::KeyringError::Read(e.to_string()))
                });
            }
            Ok(run)
        })();
        Self(created.map_err(|e| respond_value(general_log, &tool, |_| Err(e))))
    }
    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }
    fn __exit__(
        &self,
        _kind: &Bound<'_, PyAny>,
        _value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> bool {
        if let Ok(run) = &self.0 {
            let _ = sc_agent::report(run.logger(), run.close().map(|()| Value::Null));
        }
        false
    }
    #[pyo3(signature=(*args, **kwargs))]
    fn sends_to(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Py<PyAny> {
        self.with(py, |run| {
            let mut values = bind(args, kwargs, &["provider"], &[])?.into_iter();
            let provider = arg::<String>(values.next().flatten().as_ref(), "provider")?
                .ok_or_else(|| invalid("provider is required"))?;
            run.sends_to(&provider).map(Value::Bool)
        })
    }
    #[pyo3(signature=(*args, **kwargs))]
    fn send(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Py<PyAny> {
        self.with(py, |run| {
            let mut values = bind(args, kwargs, &["request"], &[])?.into_iter();
            let value = self::request(values.next().flatten().as_ref())?;
            interruptible(py, &run.cancel, || run.send(&value))
        })
    }
    #[pyo3(signature=(*args, **kwargs))]
    fn send_batch(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Py<PyAny> {
        self.with(py, |run| {
            let mut values = bind(args, kwargs, &["requests"], &["on_result"])?.into_iter();
            let requests = values.next().flatten();
            let on_result = values.next().flatten();
            let requests = required(requests.as_ref())?
                .try_iter()
                .map_err(invalid)?
                .map(|v| v.map_err(invalid).and_then(|v| request(Some(&v))))
                .collect::<Result<Vec<_>, _>>()?;
            let completed = callback(on_result.as_ref())?;
            let callback_error = Mutex::new(None);
            let on_result = |index, value: &Value| {
                if let Some(callback) = &completed {
                    Python::attach(|py| {
                        if let Err(e) = callback.call1(py, (index, raw(py, value.clone()))) {
                            run.cancel.cancel();
                            if !e.is_instance_of::<PyKeyboardInterrupt>(py)
                                && let Ok(mut first) = callback_error.lock()
                                && first.is_none()
                            {
                                *first = Some(error("sc_agent_py.callback", e));
                            }
                        }
                    });
                }
            };
            let result = interruptible(py, &run.cancel, || run.send_batch(&requests, &on_result));
            callback_result(callback_error, result)
        })
    }
    #[pyo3(signature=(*extra,**rest))]
    fn cancel(
        &self,
        py: Python<'_>,
        extra: &Bound<'_, PyTuple>,
        rest: Option<&Bound<'_, PyDict>>,
    ) -> Py<PyAny> {
        self.with(py, |run| {
            extras(extra, rest).map(|()| {
                run.cancel.cancel();
                Value::Null
            })
        })
    }
}
#[pyfunction]
fn provider_keys() -> Vec<&'static str> {
    sc_agent::PROVIDERS
        .iter()
        .map(|p| p.key)
        .filter(|k| !k.is_empty())
        .collect()
}
#[pyfunction]
fn models(py: Python<'_>) -> Py<PyAny> {
    raw(py, sc_agent::result(Ok(json!(sc_agent::PROVIDERS))))
}
#[pyfunction(signature=(*,general_log=None))]
fn languages(py: Python<'_>, general_log: Option<&Bound<'_, PyAny>>) -> Py<PyAny> {
    respond(py, general_log, "sitr", |logger| {
        sc_agent::log(logger, "sitr.languages", json!({}))?;
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
        sc_agent::log(logger, "sitr.queries", json!({}))?;
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
        sc_agent::log(logger, "sitr.query", json!({}))?;
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
        sc_agent::log(logger, "sitr.run", json!({}))?;
        Ok(value)
    })
}
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_rules! register {
        ($($function:ident => $name:expr),* $(,)?) => {$({
            let function = wrap_pyfunction!($function, module)?.unbind();
            module.add_function(PyCFunction::new_closure(
                module.py(), Some($name), None,
                move |args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>| {
                    let py = args.py();
                    match function.bind(py).call(args, kwargs) {
                        Ok(value) => value.unbind(),
                        Err(failure) => {
                            let given = kwargs.and_then(|v| v.get_item("general_log").ok().flatten());
                            let name = if matches!(stringify!($function), "languages" | "queries" | "query" | "run") { "sitr" } else { "decidr" };
                            respond(py, given.as_ref(), name, |_| Err(error("sc_agent_py.invalid_option", failure)))
                        },
                    }
                },
            )?)?;
        })*};
    }
    register!(
        logger => c"logger", log => c"log", flush => c"flush", digest => c"digest",
        provider_keys => c"provider_keys", models => c"models", languages => c"languages", queries => c"queries",
        query => c"query", run => c"run",
    );
    module.add_class::<PyLogger>()?;
    module.add_class::<PyRun>()?;
    module.add("TYPESAFE", sc_agent::TYPESAFE)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_failure_keeps_a_later_logging_failure() -> Result<(), Box<dyn std::error::Error>> {
        for outcome in [
            Err(sc_agent::error("log.flush", "controlled sink refusal")),
            Ok(
                json!([{"index":0,"result":sc_agent::result(Err(sc_agent::error("log.flush", "controlled sink refusal")))}]),
            ),
        ] {
            let failure = callback_result(
                Mutex::new(Some(sc_agent::error(
                    "sc_agent_py.callback",
                    "controlled callback refusal",
                ))),
                outcome,
            )
            .err()
            .ok_or("combined failure lost")?;
            let value = serde_json::to_value(failure)?;
            assert_eq!(value["id"], "sc_agent_py.callback");
            let message = value["message"].as_str().ok_or("message absent")?;
            for detail in [
                "controlled callback refusal",
                "log.flush",
                "controlled sink refusal",
            ] {
                assert!(message.contains(detail), "{message}");
            }
        }
        Ok(())
    }
    #[test]
    fn respond_mapping_returns_closed_logger_failure() -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("wheel-finish-{}", std::process::id()));
        let mut config = sc_observability::LoggerConfig::default_for(
            sc_observability::ServiceName::new("wheel-test")?,
            root.clone(),
        );
        config.enable_console_sink = false;
        let logger = Logger::new(config)?;
        logger.shutdown()?;
        let failure = finish(&logger, false, sc_agent::result(Ok(json!("success"))));
        assert_eq!(failure["error"]["id"], "log.flush");
        assert_eq!(
            failure["error"]["message"],
            logger
                .flush()
                .err()
                .ok_or("closed logger flushed")?
                .to_string()
        );
        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }
}
