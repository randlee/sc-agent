pub mod support;
use sc_agent::Cancel;
use serde_json::{Value, json};
use support::*;

#[test]
fn req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids() -> Test {
    // REQ-SAG-0202 / NFR-SAG-0001: one digest per question; one file, distinct session ids.
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new((0..3).map(|_| http(200, answer())).collect())?;
    let opts = options(&root, &stub.url);
    for _ in 0..3 {
        {
            let run = run(opts.clone(), &logger)?;

            run.send(&request(), Some(false))
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
        assert_eq!(row["repo"], opts.repo);
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
    assert_eq!(std::fs::read_dir(folder)?.count(), 2);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_large_request_records_or_reports_sink_limit() -> Test {
    for (size, fits) in [(1024 * 1024 - 16 * 1024, true), (1024 * 1024 + 1, false)] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let stub = Stub::new(vec![http(200, answer())])?;
        let mut input = request();
        input["state"] = json!("x".repeat(size));
        let outcome =
            sc_agent::result(run(options(&root, &stub.url), &logger)?.send(&input, Some(false)));
        let sent = stub.finish()?;
        assert_eq!(
            sent.first().ok_or("request not sent")?.body["state"],
            input["state"]
        );
        let folder = root
            .0
            .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b");
        if fits {
            assert_eq!(outcome["ok"], true, "{outcome}");
            assert_eq!(
                rows(&root)?.first().ok_or("session not written")?["request"],
                input
            );
            assert!(folder.join("runs.jsonl").exists());
        } else {
            assert_eq!(outcome["error"]["id"], "log.session_write");
            assert!(!folder.join("sessions.jsonl").exists());
            assert!(!folder.join("runs.jsonl").exists());
        }
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn j3_largest_request_that_fits_is_recorded_whole() -> Test {
    // Owner 2026-11-21: exercise sc-observability 1.5.0 directly; propagate its exact error.
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let mut event: sc_observability::LogEvent = serde_json::from_value(
        json!({"version":"v1","timestamp":"2026-01-01T00:00:00Z",
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
        let run = run(options(&root, &stub.url), &logger)?;

        run.send(&request(), Some(false))
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
    let logger = logger(&root)?;
    logger.shutdown()?;
    let cancelled = std::sync::Arc::new(Cancel::default());
    cancelled.cancel();
    let outcome = {
        let mut run = run(options(&root, "http://127.0.0.1:1"), &logger)?;
        run.cancel = cancelled.clone();
        run.keyring = Box::new(move |_| panic!("cancelled send read a key"));
        run.send(&request(), Some(false))
    };
    let failure = sc_agent::result(outcome);
    assert_eq!(failure["error"]["id"], "decision.cancelled");
    let session_id = failure["error"]["session_id"]
        .as_str()
        .ok_or("missing session id")?
        .to_owned();
    for session in [None, Some(session_id)] {
        let failure = sc_agent::report(
            &logger,
            Err(sc_agent::Error::Failure {
                id: "test.failure",
                message: "reported error".into(),
                action: "Retry later.",
                category: "other",
                attempts: None,
                session_id: session.clone(),
            }),
        );
        assert_eq!(failure["error"]["id"], "test.failure");
        assert_eq!(failure["error"]["message"], "reported error");
        assert_eq!(failure["error"]["action"], "Retry later.");
        assert_eq!(failure["error"]["category"], "other");
        assert_eq!(
            failure["error"].get("session_id"),
            session.as_ref().map(|id| json!(id)).as_ref()
        );
    }
    Ok(())
}

#[test]
fn req_sag_0019_missing_runner_names_its_setting_layers() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let mut opts = options(&root, "http://127.0.0.1:1");
    opts.model = "laya".into();
    let failure = sc_agent::result({
        let run = run(opts.clone(), &logger)?;

        run.send(&request(), Some(false))
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
    let logger = logger(&root)?;
    logger.shutdown()?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let failure =
        sc_agent::result(run(options(&root, &stub.url), &logger)?.send(&request(), Some(false)));
    assert_eq!(failure["error"]["id"], "log.write");
    assert!(failure["error"]["session_id"].as_str().is_some());
    assert!(
        failure["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Ok("))
    );
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
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer())])?;
    {
        let run = run(options(&root, &stub.url), &logger)?;

        run.send(&request(), Some(false))
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
    fail_flush: bool,
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
    fn flush(&self) -> Result<(), sc_observability::v2::LogSinkError> {
        if self.fail_flush {
            return Err(sc_observability::v2::LogSinkError::Flush {
                context: Box::new(sc_observability::ErrorContext::new(
                    sc_observability::error_codes::LOGGER_SINK_WRITE_FAILED,
                    "controlled flush failure",
                    sc_observability::Remediation::not_recoverable("test flush"),
                )),
            });
        }
        Ok(())
    }
}
#[test]
fn session_write_failure_survives_later_flush_failure() -> Test {
    let root = Temp::new()?;
    std::fs::write(root.0.join("sessions"), "not a directory")?;
    let io_error = std::fs::create_dir_all(root.0.join("sessions/example"))
        .err()
        .ok_or("expected directory error")?
        .to_string();
    let sink = std::sync::Arc::new(Captures {
        events: Default::default(),
        name: sc_observability::SinkName::new("flush-failure")?,
        fail_flush: true,
    });
    let mut config = sc_observability::LoggerConfig::default_for(
        sc_observability::ServiceName::new("decidr")?,
        root.0.clone(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = sc_observability::v2::Logger::builder(config)?;
    builder.register_sink(sc_observability::SinkRegistration::typed(sink.clone()));
    let logger = builder.build()?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let logger = std::sync::Arc::new(logger);
    let failure =
        sc_agent::result(run(options(&root, &stub.url), &logger)?.send(&request(), Some(false)));
    assert_eq!(failure["error"]["id"], "log.session_write");
    assert!(failure["error"]["session_id"].as_str().is_some());
    let message = failure["error"]["message"]
        .as_str()
        .ok_or("missing message")?;
    assert!(message.contains(&io_error), "{message}");
    assert!(
        sink.events
            .lock()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|event| event.fields.get("id") == Some(&json!("log.session_write")))
    );
    assert_eq!(stub.finish()?.len(), 1);
    Ok(())
}
#[test]
fn injected_test_sink_receives_every_general_record_without_default_file() -> Test {
    let root = Temp::new()?;
    let sink = std::sync::Arc::new(Captures {
        events: Default::default(),
        name: sc_observability::SinkName::new("capture")?,
        fail_flush: false,
    });
    let mut config = sc_observability::LoggerConfig::default_for(
        sc_observability::ServiceName::new("decidr")?,
        root.0.clone(),
    );
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = sc_observability::v2::Logger::builder(config)?;
    builder.register_sink(sc_observability::SinkRegistration::typed(sink.clone()));
    let logger = std::sync::Arc::new(builder.build()?);
    let stub = Stub::new(vec![http(200, answer())])?;
    {
        let run = run(options(&root, &stub.url), &logger)?;

        run.send(&request(), Some(false))
    }?;
    assert_eq!(stub.finish()?.len(), 1);
    sc_agent::report(
        &logger,
        Err(sc_agent::error("test.error", "controlled failure")),
    );
    sc_agent::log_as(
        &logger,
        "consumer",
        "client.event",
        Some("correlation-1"),
        json!({"key":"value"}),
    )?;
    logger.flush()?;
    let events = sink.events.lock().map_err(|e| e.to_string())?;
    assert_eq!(events.len(), 3);
    let serialized = events
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(serialized[0]["action"], "decision.result");
    let fields = serialized[0]["fields"]
        .as_object()
        .ok_or("result fields missing")?;
    assert_eq!(
        fields.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["cached", "session_id"]
    );
    assert_eq!(serialized[1]["fields"]["id"], "test.error");
    assert_eq!(serialized[2]["target"], "consumer");
    assert_eq!(serialized[2]["action"], "client.event");
    assert_eq!(serialized[2]["correlation_id"], "correlation-1");
    assert_eq!(serialized[2]["fields"]["key"], "value");
    assert!(!root.0.join("logs/decidr.log.jsonl").exists());
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_row_and_run_link_capture_identity_and_body_hash() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let run_id = run.run_id.to_string();
    let value = run.send(&request(), Some(false))?;
    let captured = stub.finish()?;
    let rows = rows(&root)?;
    let row = rows.first().ok_or("missing session row")?;
    assert_eq!(row["schema"], "sc-agent-session/2");
    assert_eq!(row["tool"], "decidr");
    assert_eq!(row["tool_version"], "test");
    assert_eq!(row["provider"], "typesafe");
    assert_eq!(row["model_requested"], "jev-latest");
    assert_eq!(row["model_answered"], "jev-1.14.0");
    assert_eq!(row["usage"], Value::Null);
    assert_eq!(
        row["request_sha256"],
        sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
            utf8_file_bytes: &serde_json::to_vec(&captured.first().ok_or("request missing")?.body)?
        })?
        .template()
        .to_string()
    );
    assert!(row.get("run_id").is_none());
    assert_eq!(value["cached"], false);
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/runs.jsonl");
    let link: Value = serde_json::from_str(std::fs::read_to_string(path)?.trim())?;
    assert_eq!(
        link["fields"],
        json!({"run_id":run_id,"session_id":value["session_id"],"cached":false})
    );
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_sinks_never_rotate() -> Test {
    use std::fs::{File, FileTimes};
    use std::time::SystemTime;
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let folder = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b");
    std::fs::create_dir_all(&folder)?;
    let initial = sc_observability::RetainedLogPolicy::default()
        .rotation_max_bytes
        .as_u64()
        + 1;
    for name in ["sessions.jsonl", "runs.jsonl"] {
        let file = File::create(folder.join(name))?;
        file.set_len(initial)?;
        file.set_times(FileTimes::new().set_modified(SystemTime::UNIX_EPOCH))?;
    }
    let stub = Stub::new(vec![http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    assert!(run.send(&request(), Some(false)).is_ok());
    assert_eq!(stub.finish()?.len(), 1);
    for name in ["sessions.jsonl", "runs.jsonl"] {
        assert!(std::fs::metadata(folder.join(name))?.len() > initial);
    }
    assert_eq!(std::fs::read_dir(folder)?.count(), 2);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_link_write_failure() -> Test {
    for status in [200, 401] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let folder = root
            .0
            .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b");
        std::fs::create_dir_all(folder.join("runs.jsonl"))?;
        let body = if status == 200 {
            answer()
        } else {
            json!({"error":"denied"})
        };
        let stub = Stub::new(vec![http(status, body.clone())])?;
        let run = run(options(&root, &stub.url), &logger)?;
        let outcome = sc_agent::report(&logger, run.send(&request(), Some(false)));
        assert_eq!(
            outcome["error"]["id"],
            if status == 200 {
                "log.session_write"
            } else {
                "http.auth"
            }
        );
        if status == 401 {
            assert_eq!(outcome["error"]["message"], body.to_string());
        }
        assert_eq!(stub.finish()?.len(), 1);
        if status == 200 {
            assert_eq!(rows(&root)?.len(), 1);
        } else {
            assert!(!folder.join("sessions.jsonl").exists());
        }
        assert!(folder.join("runs.jsonl").is_dir());
        logger.flush()?;
        let log = std::fs::read_to_string(root.0.join("logs/rust-test.log.jsonl"))?;
        let errors = log
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            errors
                .iter()
                .filter(|event| event.pointer("/fields/id").and_then(Value::as_str)
                    == Some("log.session_write"))
                .count(),
            usize::from(status == 200)
        );
        run.close()?;
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn j3_batch_links() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new((0..3).map(|_| http(200, answer())).collect())?;
    let run = run(options(&root, &stub.url), &logger)?;
    let result = run.send_batch(&vec![request(); 3], Some(false), &|_, _| {})?;
    let result = result.as_array().ok_or("batch missing")?;
    assert_eq!(result.len(), 3);
    assert_eq!(stub.finish()?.len(), 3);
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/runs.jsonl");
    let links = std::fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(links.len(), 3);
    for (row, link) in result.iter().zip(links.iter()) {
        assert_eq!(link["fields"]["run_id"], run.run_id.to_string());
        assert_eq!(
            link["fields"]["session_id"],
            row["result"]["value"]["session_id"]
        );
        assert_eq!(link["fields"]["cached"], false);
    }
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_interrupt_keeps_finished_rows() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let result = run.send_batch(&vec![request(); 3], Some(false), &|index, _| {
        if index == 0 {
            run.cancel.cancel();
        }
    })?;
    let values = result.as_array().ok_or("batch missing")?;
    assert_eq!(values.len(), 3);
    assert_eq!(values.first().ok_or("first missing")?["result"]["ok"], true);
    for value in values.iter().skip(1) {
        assert_eq!(value["result"]["error"]["id"], "decision.cancelled");
    }
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(rows(&root)?.len(), 1);
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/runs.jsonl");
    assert_eq!(std::fs::read_to_string(path)?.lines().count(), 1);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_oversized_provider_error_creates_no_session_data() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let message = "x".repeat(2 * 1024 * 1024);
    let body = json!({"error":message});
    let stub = Stub::new(vec![http(401, body.clone())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let result = sc_agent::result(run.send(&request(), Some(false)));
    assert_eq!(result["error"]["id"], "http.auth");
    assert_eq!(result["error"]["message"], body.to_string());
    assert_eq!(stub.finish()?.len(), 1);
    assert!(!root.0.join("sessions").exists());
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_report_preserves_oversized_provider_error() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let message = "x".repeat(2 * 1024 * 1024);
    let reported = sc_agent::report(&logger, Err(sc_agent::error("http.auth", &message)));
    assert_eq!(reported["error"]["id"], "http.auth");
    assert_eq!(reported["error"]["message"], message);
    logger.shutdown()?;
    Ok(())
}
