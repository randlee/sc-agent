//! The shared bridge to a caller-owned Python Logger or record callable.
use crate::RecordSink;
use pyo3::{prelude::*, types::PyDict};
use sc_agent_types::Error;
use sc_observability::{ServiceName, v2::LogEvent};
use std::sync::Arc;

pub struct PythonSink {
    target: Py<PyAny>,
    service: ServiceName,
}
impl PythonSink {
    pub fn open(target: Py<PyAny>) -> Result<Arc<dyn RecordSink>, Error> {
        Ok(Arc::new(Self {
            target,
            service: ServiceName::new("decidr").map_err(|e| Error::Log(e.to_string()))?,
        }))
    }
}
impl RecordSink for PythonSink {
    fn service(&self) -> &ServiceName {
        &self.service
    }
    fn write(&self, event: LogEvent) -> Result<(), Error> {
        Python::attach(|py| -> PyResult<()> {
            let payload = serde_json::json!({ "level": "info", "target": event.target,
                "action": event.action, "fields": event.fields });
            let record = py
                .import("json")?
                .call_method1("loads", (payload.to_string(),))?;
            let target = self.target.bind(py);
            if target.is_callable() {
                target.call1((record,))?;
            } else {
                let obs = py.import("sc_observability")?;
                let event = obs
                    .getattr("LogEvent")?
                    .call((), Some(record.cast::<PyDict>()?))?;
                let result = target.call_method1("log", (event,))?;
                if result.getattr("kind")?.extract::<String>()? != "ok" {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        result.str()?.to_string(),
                    ));
                }
            }
            Ok(())
        })
        .map_err(|e| Error::Log(e.to_string()))
    }
}
