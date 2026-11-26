pub mod support;
use sc_agent::{Call, Cancel, Run};
use serde_json::{Value, json};
use std::sync::Arc;
use support::*;

#[test]
fn req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids() -> Test {
    // REQ-SAG-0202 / NFR-SAG-0001: one digest per question; one file, distinct session ids.
    let root = Temp::new()?;
    let logger = Arc::new(logger(&root)?);
    let stub = Stub::new((0..3).map(|_| http(200, answer())).collect())?;
    let opts = run_config(&root, &stub)?;
    for _ in 0..3 {
        {
            let mut cfg = (opts).clone();
            cfg.logger = Some(logger.clone());
            cfg.cancel = Some(Arc::new(Cancel::default()));
            Run::new(cfg).and_then(|run| {
                let result = run.send(&Call::default(), &request());
                let closed = run.close();
                result.and_then(|value| closed.map(|()| value))
            })
        }?;
    }
    let seen = stub.finish()?;
    assert_eq!(seen.len(), 3);
    let rows = rows(&root)?;
    assert_eq!(rows.len(), 3);
    let ids = rows
        .iter()
        .map(|row| {
            row["session_id"]
                .as_str()
                .ok_or("missing session id")?
                .parse::<ulid::Ulid>()
                .map_err(Into::into)
        })
        .collect::<Result<std::collections::BTreeSet<_>, Box<dyn std::error::Error + Send + Sync>>>(
        )?;
    assert_eq!(ids.len(), 3);
    for row in &rows {
        assert_eq!(row["repo"], "github.com/randlee/sc-agent");
        assert_eq!(row["project"], "phase-b");
        assert_eq!(row["agent_role"], "rust-developer");
        assert_eq!(row["request"], request());
        assert_eq!(
            row["question_digests"]["q"],
            sc_agent::digest(&request(), "q")?
        );
        assert_eq!(row["result"]["value"]["answers"]["q"]["noul"], 0.123456789);
    }
    let folder = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b");
    assert_eq!(std::fs::read_dir(folder)?.count(), 1);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0005_one_mib_request_is_recorded_whole() -> Test {
    // Owner 2026-11-21: accept a 1 MiB request and read the session row back whole.
    let root = Temp::new()?;
    let logger = Arc::new(logger(&root)?);
    let stub = Stub::new(vec![http(200, answer())])?;
    let mut input = request();
    input["state"] = json!("");
    let payload = "x".repeat(1024 * 1024 - serde_json::to_vec(&input)?.len());
    input["state"] = json!(payload);
    assert_eq!(serde_json::to_vec(&input)?.len(), 1024 * 1024);
    {
        let mut cfg = (run_config(&root, &stub)?).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(Arc::new(Cancel::default()));
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &input);
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    }?;
    assert_eq!(
        stub.finish()?.first().ok_or("request not sent")?.body["state"],
        input["state"]
    );
    assert_eq!(
        rows(&root)?.first().ok_or("session not written")?["request"],
        input
    );
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0008_one_mib_jsonl_limit_preserves_accepted_event_and_decision() -> Test {
    // Owner 2026-11-21: exercise sc-observability 1.5.0 directly; propagate its exact error.
    let root = Temp::new()?;
    let logger = Arc::new(logger(&root)?);
    let mut event: sc_observability::LogEvent = serde_json::from_value(
        json!({"version":"v1","timestamp":sc_observability::Timestamp::now_utc(),
        "identity":sc_observability::ProcessIdentity::default(), "service":logger.service_name(),
        "level":sc_observability::Level::Info,"target":"rust-test","action":"large-event","fields":{"payload":""}}),
    )?;
    let payload = "x".repeat(1024 * 1024 - 1 - serde_json::to_vec(&event)?.len());
    event.fields.insert("payload".into(), json!(payload));
    assert_eq!(serde_json::to_vec(&event)?.len() + 1, 1024 * 1024);
    logger.log(event.clone())?;
    let mut oversized = event.clone();
    oversized
        .fields
        .insert("payload".into(), json!(format!("{payload}x")));
    assert_eq!(serde_json::to_vec(&oversized)?.len(), 1024 * 1024);
    let refusal = logger
        .log(oversized)
        .err()
        .ok_or("oversized event accepted")?;
    assert_eq!(
        refusal.diagnostic().code.as_str(),
        "SC_OBSERVABILITY_LOGGER_INVALID_EVENT"
    );
    logger.flush()?;
    let text = std::fs::read_to_string(root.0.join("logs/rust-test.log.jsonl"))?;
    let written = text
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(written.len(), 1);
    assert_eq!(text.len(), 1024 * 1024);
    assert_eq!(
        written.first().ok_or("log event not written")?["fields"]["payload"],
        event
            .fields
            .get("payload")
            .ok_or("payload missing")?
            .clone()
    );
    let stub = Stub::new(vec![http(200, answer())])?;
    let value = {
        let mut cfg = (run_config(&root, &stub)?).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(Arc::new(Cancel::default()));
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &request());
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    }?;
    assert_eq!(value["answers"], answer()["answers"]);
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(
        rows(&root)?.first().ok_or("decision session missing")?["result"]["ok"],
        true
    );
    logger.shutdown()?;
    Ok(())
}

#[test]
fn adr_sag_0021_send_and_report_return_logger_failures() -> Test {
    let root = Temp::new()?;
    let logger = Arc::new(logger(&root)?);
    logger.shutdown()?;
    let cancelled = Arc::new(Cancel::default());
    cancelled.cancel();
    let outcome = {
        let mut cfg = (run_config(&root, &Stub::new(vec![])?)?).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(cancelled.clone());
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &request());
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    };
    let failure = sc_agent::result(outcome);
    assert_eq!(failure["error"]["id"], "log.flush");
    let original = logger
        .flush()
        .err()
        .ok_or("closed logger flushed")?
        .to_string();
    assert_eq!(failure["error"]["message"], original);
    let failure = sc_agent::report(
        &logger,
        Err(sc_agent::error("test.failure", "reported error")),
    );
    assert_eq!(failure["error"]["id"], "log.write");
    let original =
        sc_agent::result(sc_agent::log(&logger, "error", json!({})).map(|()| json!(null)));
    assert_eq!(failure["error"], original["error"]);
    Ok(())
}

#[test]
fn req_sag_0019_missing_runner_names_its_setting_layers() -> Test {
    let root = Temp::new()?;
    let logger = Arc::new(logger(&root)?);
    let mut opts = run_config(&root, &Stub::new(vec![])?)?;
    opts.model = Some("laya".into());
    let failure = sc_agent::result({
        let mut cfg = (opts).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(Arc::new(Cancel::default()));
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &request());
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    });
    assert_eq!(failure["error"]["id"], "config.missing_setting");
    let message = failure["error"]["message"]
        .as_str()
        .ok_or("missing message")?;
    for layer in [
        "laya",
        "DECIDR_LAYA__HOST",
        "local decidr.json",
        "local sc-agent.json",
        "global decidr.json",
        "global sc-agent.json",
    ] {
        assert!(message.contains(layer), "{message}");
    }
    logger.shutdown()?;
    Ok(())
}

#[test]
fn failed_general_logger_preserves_the_successful_session() -> Test {
    let root = Temp::new()?;
    let logger = Arc::new(logger(&root)?);
    logger.shutdown()?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let failure = sc_agent::result({
        let mut cfg = (run_config(&root, &stub)?).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(Arc::new(Cancel::default()));
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &request());
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    });
    assert_eq!(failure["error"]["id"], "log.write");
    assert_eq!(stub.finish()?.len(), 1);
    let sessions = rows(&root)?;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["result"]["ok"], true);
    assert_eq!(
        sessions[0]["result"]["value"]["answers"],
        answer()["answers"]
    );
    Ok(())
}

#[test]
fn old_session_file_survives_a_new_decision_unchanged() -> Test {
    let root = Temp::new()?;
    let old = root.0.join("sessions/example/old/project/sessions.jsonl");
    std::fs::create_dir_all(old.parent().ok_or("missing parent")?)?;
    std::fs::write(&old, "{\"old\":true}\n")?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&old)?
        .set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))?;
    let modified = std::fs::metadata(&old)?.modified()?;
    let logger = Arc::new(logger(&root)?);
    let stub = Stub::new(vec![http(200, answer())])?;
    {
        let mut cfg = (run_config(&root, &stub)?).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(Arc::new(Cancel::default()));
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &request());
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    }?;
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(std::fs::read_to_string(&old)?, "{\"old\":true}\n");
    assert_eq!(std::fs::metadata(&old)?.modified()?, modified);
    assert_eq!(
        std::fs::read_dir(root.0.join("logs"))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == "sessions.jsonl")
            .count(),
        0
    );
    logger.shutdown()?;
    Ok(())
}

struct Captures {
    events: std::sync::Mutex<Vec<sc_observability::LogEvent>>,
    name: sc_observability::SinkName,
}
impl sc_observability::v2::LogSink for Captures {
    fn write(
        &self,
        event: &sc_observability::LogEvent,
    ) -> Result<(), sc_observability::v2::LogSinkError> {
        self.events
            .lock()
            .map(|mut events| events.push(event.clone()))
            .map_err(|e| sc_observability::v2::LogSinkError::Write {
                context: Box::new(sc_observability::ErrorContext::new(
                    sc_observability::error_codes::LOGGER_SINK_WRITE_FAILED,
                    e.to_string(),
                    sc_observability::Remediation::not_recoverable("test sink lock poisoned"),
                )),
            })
    }
    fn health(&self) -> sc_observability::SinkHealth {
        sc_observability::SinkHealth {
            name: self.name.clone(),
            state: sc_observability::SinkHealthState::Healthy,
            last_error: None,
        }
    }
}
#[test]
fn injected_test_sink_receives_every_general_record_without_default_file() -> Test {
    let root = Temp::new()?;
    let sink = std::sync::Arc::new(Captures {
        events: Default::default(),
        name: sc_observability::SinkName::new("capture")?,
    });
    let mut config = sc_observability::LoggerConfig::default_for(
        sc_observability::ServiceName::new("decidr")?,
        root.0.clone(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = sc_observability::v2::Logger::builder(config)?;
    builder.register_sink(sc_observability::SinkRegistration::typed(sink.clone()));
    let logger = Arc::new(builder.build()?);
    let stub = Stub::new(vec![http(200, answer())])?;
    {
        let mut cfg = (run_config(&root, &stub)?).clone();
        cfg.logger = Some(logger.clone());
        cfg.cancel = Some(Arc::new(Cancel::default()));
        Run::new(cfg).and_then(|run| {
            let result = run.send(&Call::default(), &request());
            let closed = run.close();
            result.and_then(|value| closed.map(|()| value))
        })
    }?;
    assert_eq!(stub.finish()?.len(), 1);
    sc_agent::report(
        &logger,
        Err(sc_agent::error("test.error", "controlled failure")),
    );
    logger.flush()?;
    let events = sink.events.lock().map_err(|e| e.to_string())?;
    assert_eq!(events.len(), 2);
    let serialized = events
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(serialized[0]["action"], "decision.result");
    assert_eq!(serialized[1]["fields"]["id"], "test.error");
    assert!(!root.0.join("logs/decidr.log.jsonl").exists());
    logger.shutdown()?;
    Ok(())
}
