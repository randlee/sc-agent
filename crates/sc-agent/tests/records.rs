pub mod support;
use sc_agent::{Cancel, send};
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
        send(
            &request(),
            &opts,
            &logger,
            &Cancel::default(),
            &no_wait,
            &keyring,
        )?;
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
    assert_eq!(std::fs::read_dir(folder)?.count(), 1);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0005_one_mib_request_is_recorded_whole() -> Test {
    // Owner 2026-11-21: accept a 1 MiB request and read the session row back whole.
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let mut input = request();
    input["state"] = json!("");
    let payload = "x".repeat(1024 * 1024 - serde_json::to_vec(&input)?.len());
    input["state"] = json!(payload);
    assert_eq!(serde_json::to_vec(&input)?.len(), 1024 * 1024);
    send(
        &input,
        &options(&root, &stub.url),
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    )?;
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
    let logger = logger(&root)?;
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
    let value = send(
        &request(),
        &options(&root, &stub.url),
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    )?;
    assert_eq!(value["answers"], answer()["answers"]);
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(
        rows(&root)?.first().ok_or("decision session missing")?["result"]["ok"],
        true
    );
    logger.shutdown()?;
    Ok(())
}
