use sc_agent_log::{RecordSink, Session};
use sc_agent_types::Error;
use sc_observability::{ServiceName, v2::LogEvent};
use serde_json::Map;
use std::sync::{Arc, Mutex};

struct Capture {
    service: ServiceName,
    events: Mutex<Vec<LogEvent>>,
}
impl RecordSink for Capture {
    fn service(&self) -> &ServiceName {
        &self.service
    }
    fn write(&self, event: LogEvent) -> Result<(), Error> {
        self.events
            .lock()
            .map_err(|e| Error::Log(e.to_string()))?
            .push(event);
        Ok(())
    }
}
#[test]
fn injected_session_goes_only_to_its_sink() -> Result<(), Box<dyn std::error::Error>> {
    let sink = Arc::new(Capture {
        service: ServiceName::new("app")?,
        events: Mutex::new(Vec::new()),
    });
    let session = Session::injected(
        sink.clone(),
        "host/owner/repo".into(),
        "phase-h",
        "consumer",
    )?;
    session.row(Map::new());
    drop(session);
    let events = sink.events.lock().map_err(|e| Error::Log(e.to_string()))?;
    assert_eq!(events.len(), 1);
    let event = events.first().ok_or("missing event")?;
    assert_eq!(event.target.as_str(), "sc_agent.session");
    assert_eq!(event.fields.get("project"), Some(&"phase-h".into()));
    assert_eq!(event.fields.get("agent_role"), Some(&"consumer".into()));
    Ok(())
}
