pub mod support;
use sc_agent::providers::Wire;
use sc_agent::{Call, Cancel, Environment, Run, Secret, host_holdoff};
use serde_json::{Value, json};
use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use support::*;

#[test]
fn req_sag_0201_0007_provider_wires_and_responses_preserve_values() -> Test {
    for (provider, model, id, wire) in [
        ("typesafe", "jev", "jev-latest", Wire::SystemOne),
        ("anthropic", "haiku", "claude-haiku-5-5", Wire::Messages),
        ("openai", "luna", "gpt-6-luna", Wire::ChatCompletions),
        (
            "openrouter",
            "haiku",
            "anthropic/claude-haiku-5.5",
            Wire::ChatCompletions,
        ),
        (
            "together",
            "luna",
            "openai/gpt-6-luna",
            Wire::ChatCompletions,
        ),
        ("nous", "luna", "openai/gpt-6-luna", Wire::ChatCompletions),
    ] {
        let root = Temp::new()?;
        let raw = match wire {
            Wire::SystemOne => answer(),
            Wire::Messages => json!({"content":[{"type":"text","text":answer().to_string()}]}),
            Wire::ChatCompletions => {
                json!({"choices":[{"message":{"content":answer().to_string()}}]})
            }
        };
        fs::write(
            root.0.join("sc-agent.json"),
            json!({"project":"phase-b","models":{model:{"provider":provider,"id":id}}}).to_string(),
        )?;
        let stub = Stub::new(vec![http(200, raw)])?;
        let mut cfg = run_config(&root, &stub)?;
        cfg.model = Some(model.into());
        let mut env = cfg
            .seams
            .environment
            .take()
            .unwrap_or_default()
            .0
            .expose()
            .clone();
        env.extend([
            ("OPENROUTER_API_KEY".into(), "test-secret".into()),
            ("TOGETHER_API_KEY".into(), "test-secret".into()),
            ("NOUS_API_KEY".into(), "test-secret".into()),
        ]);
        cfg.seams.environment = Some(Environment(Secret::new(env)));
        let run = Run::new(cfg)?;
        let value = run.send(&Call::default(), &request())?;
        assert_eq!(value["answers"], answer()["answers"]);
        assert_eq!(value["answers"]["q"]["noul"], 0.123456789);
        let seen = stub.finish()?;
        let first = seen.first().ok_or("request missing")?;
        assert_eq!(first.path, format!("/v1/{}", wire.path()));
        assert_eq!(first.body["model"], id);
        match wire {
            Wire::SystemOne => {
                let mut expected = request();
                expected["model"] = json!(id);
                assert_eq!(first.body, expected);
            }
            _ => {
                let schema = match wire {
                    Wire::Messages => &first.body["output_config"]["format"]["schema"],
                    _ => &first.body["response_format"]["json_schema"]["schema"],
                };
                assert_eq!(schema["required"], json!(["answers"]));
                assert_eq!(schema["properties"]["answers"]["required"], json!(["q"]));
                assert_eq!(
                    schema["properties"]["answers"]["properties"]["q"]["properties"]["type"]["enum"],
                    json!(["noul"])
                );
                assert!(first.body["messages"].to_string().contains("evidence"));
            }
        }
        let auth = match wire {
            Wire::Messages => ("x-api-key", "test-secret"),
            _ => ("authorization", "Bearer test-secret"),
        };
        assert!(
            first
                .headers
                .iter()
                .any(|(k, v)| (k.as_str(), v.as_str()) == auth)
        );
        if wire == Wire::Messages {
            assert!(
                first
                    .headers
                    .iter()
                    .any(|(k, v)| k == "anthropic-version" && v == "2023-06-01")
            );
        }
        run.close()?;
    }
    Ok(())
}
#[test]
fn req_sag_0601_busy_resends_unchanged_retry_after_and_exponential_backoff() -> Test {
    for status in [429, 502, 529] {
        let root = Temp::new()?;
        let stub = Stub::new(vec![
            Reply::Http(status, vec![("Retry-After", "1.1")], "busy".into()),
            http(status, json!({})),
            Reply::Http(status, vec![("Retry-After", "malformed")], "busy".into()),
            http(200, answer()),
        ])?;
        let mut cfg = run_config(&root, &stub)?;
        let waits = Arc::new(Mutex::new(vec![]));
        let recorded = waits.clone();
        cfg.seams.wait = Arc::new(move |delay, _| {
            if let Ok(mut w) = recorded.lock() {
                w.push(delay);
            }
        });
        let run = Run::new(cfg)?;
        let value = run.send(&Call::default(), &request())?;
        assert_eq!(value["attempts"], 4);
        assert_eq!(
            *waits.lock().map_err(|e| e.to_string())?,
            vec![
                Duration::from_secs_f64(1.1),
                Duration::from_secs(2),
                Duration::from_secs(4)
            ]
        );
        let seen = stub.finish()?;
        assert_eq!(seen.len(), 4);
        for sent in &seen {
            assert_eq!(sent.body, seen.first().ok_or("no sends")?.body);
        }
        assert_eq!(rows(&root)?.len(), 1);
        run.close()?;
    }
    Ok(())
}
#[test]
fn req_sag_0601_0017_busy_and_network_exhaustion_report_five_sends() -> Test {
    for network in [false, true] {
        let root = Temp::new()?;
        let stub = Stub::new(
            (0..5)
                .map(|_| {
                    if network {
                        Reply::Disconnect
                    } else {
                        http(529, json!({"error":"busy"}))
                    }
                })
                .collect(),
        )?;
        let waits = Arc::new(Mutex::new(vec![]));
        let captured = waits.clone();
        let mut cfg = run_config(&root, &stub)?;
        cfg.seams.wait = Arc::new(move |delay, _| {
            if let Ok(mut w) = captured.lock() {
                w.push(delay);
            }
        });
        let run = Run::new(cfg)?;
        let got = sc_agent::result(run.send(&Call::default(), &request()));
        assert_eq!(
            got["error"]["id"],
            if network {
                "http.network"
            } else {
                "http.provider_busy"
            }
        );
        assert_eq!(
            got["error"]["category"],
            if network { "network" } else { "busy" }
        );
        assert_eq!(got["error"]["attempts"], 5);
        assert_eq!(got["error"]["action"], "Retry later.");
        assert_eq!(stub.finish()?.len(), 5);
        assert_eq!(
            *waits.lock().map_err(|e| e.to_string())?,
            [1, 2, 4, 8].map(Duration::from_secs)
        );
        run.close()?;
    }
    Ok(())
}
#[test]
fn req_sag_0002_0601_auth_quota_are_not_retried_and_remote_text_is_redacted_whole() -> Test {
    for (status, body, id, category) in [
        (
            401,
            "complete prefix test-secret exact suffix",
            "http.auth",
            "auth",
        ),
        (
            402,
            "complete quota response",
            "http.quota_exhausted",
            "quota",
        ),
        (
            429,
            "insufficient_quota unchanged response",
            "http.quota_exhausted",
            "quota",
        ),
        (
            529,
            "credit balance exhausted",
            "http.quota_exhausted",
            "quota",
        ),
    ] {
        let root = Temp::new()?;
        let logger = Arc::new(logger(&root)?);
        let stub = Stub::new(vec![Reply::Http(status, vec![], body.into())])?;
        let mut cfg = run_config(&root, &stub)?;
        cfg.logger = Some(logger.clone());
        let run = Run::new(cfg)?;
        let got = sc_agent::report(&logger, run.send(&Call::default(), &request()));
        assert_eq!(got["error"]["id"], id);
        assert_eq!(got["error"]["category"], category);
        assert_eq!(
            got["error"]["message"],
            body.replace("test-secret", "[redacted]")
        );
        assert_eq!(got["error"]["attempts"], 1);
        assert_eq!(stub.finish()?.len(), 1);
        logger.flush()?;
        let general = fs::read_to_string(root.0.join("logs/rust-test.log.jsonl"))?;
        assert!(!general.contains("test-secret"));
        assert!(!serde_json::to_string(&rows(&root)?)?.contains("test-secret"));
        let errors = general
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            errors
                .iter()
                .filter(|e| e["action"] == "error" && e["fields"]["id"] == id)
                .count(),
            1
        );
        run.close()?;
        logger.shutdown()?;
    }
    Ok(())
}
#[test]
fn req_sag_0601_holdoff_lock_is_shared_only_by_host_and_held_during_wait() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![http(429, json!({})), http(200, answer())])?;
    let host = host_holdoff("127.0.0.1")?;
    assert!(Arc::ptr_eq(&host, &host_holdoff("127.0.0.1")?));
    let other = host_holdoff("independent.invalid")?;
    assert!(!Arc::ptr_eq(&host, &other));
    let observations = Arc::new(Mutex::new(vec![]));
    let captured = observations.clone();
    let mut cfg = run_config(&root, &stub)?;
    cfg.seams.wait = Arc::new(move |_, _| {
        let blocked = matches!(host.try_lock(), Err(std::sync::TryLockError::WouldBlock));
        let independent = other.try_lock().is_ok();
        if let Ok(mut seen) = captured.lock() {
            seen.push((blocked, independent));
        }
    });
    let run = Run::new(cfg)?;
    run.send(&Call::default(), &request())?;
    assert_eq!(
        *observations.lock().map_err(|e| e.to_string())?,
        vec![(true, true)]
    );
    assert_eq!(stub.finish()?.len(), 2);
    run.close()?;
    Ok(())
}
#[test]
fn req_sag_0010_0011_cancel_during_retry_keeps_every_batch_position_and_row() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![Reply::Http(
        429,
        vec![("Retry-After", "600")],
        "busy".into(),
    )])?;
    let waits = Arc::new(Mutex::new(vec![]));
    let capture = waits.clone();
    let mut cfg = run_config(&root, &stub)?;
    cfg.seams.wait = Arc::new(move |delay, cancel| {
        if let Ok(mut w) = capture.lock() {
            w.push(delay);
        }
        cancel.cancel();
        cancel.wait(Duration::from_secs(600));
    });
    let run = Run::new(cfg)?;
    let completed = Mutex::new(vec![]);
    let got = run.send_batch(&Call::default(), &vec![request(); 3], &|index, value| {
        if let Ok(mut c) = completed.lock() {
            c.push((index, value.clone()));
        }
    })?;
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(got.as_array().ok_or("missing results")?.len(), 3);
    for (index, result) in got.as_array().ok_or("missing results")?.iter().enumerate() {
        assert_eq!(result["index"], index);
        assert_eq!(result["result"]["error"]["id"], "decision.cancelled");
        assert_eq!(
            result["result"]["error"]["attempts"],
            if index == 0 { 1 } else { 0 }
        );
        result["result"]["error"]["session_id"]
            .as_str()
            .ok_or("session id absent")?
            .parse::<ulid::Ulid>()?;
    }
    assert_eq!(completed.into_inner().map_err(|e| e.to_string())?.len(), 3);
    assert_eq!(
        *waits.lock().map_err(|e| e.to_string())?,
        vec![Duration::from_secs(600)]
    );
    assert_eq!(rows(&root)?.len(), 3);
    run.close()?;
    Ok(())
}
#[test]
fn req_sag_0011_precancel_never_reads_a_key_and_invalid_tags_write_no_row() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![])?;
    let mut cfg = run_config(&root, &stub)?;
    let keys = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let captured = keys.clone();
    cfg.seams.keyring = Arc::new(move |_| {
        captured.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(sc_agent::KeyringError::Read("unexpected lookup".into()))
    });
    let run = Run::new(cfg)?;
    run.cancel();
    let got = sc_agent::result(run.send(&Call::default(), &request()));
    assert_eq!(got["error"]["id"], "decision.cancelled");
    assert_eq!(keys.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(rows(&root)?.len(), 1);
    run.close()?;
    for invalid_role in [false, true] {
        let isolated = Temp::new()?;
        let mut cfg = run_config(&isolated, &stub)?;
        if invalid_role {
            cfg.agent_role = Some("Phase B".into());
        } else {
            cfg.project = Some("Phase B".into());
        }
        let run = Run::new(cfg)?;
        let error = run
            .send(&Call::default(), &request())
            .err()
            .ok_or("invalid tag accepted")?;
        assert_eq!(error_id(&error), "log.invalid_slug");
        assert!(!isolated.0.join("sessions").exists());
        run.close()?;
    }
    assert!(stub.finish()?.is_empty());
    Ok(())
}
#[test]
fn req_sag_0702_batch_continues_after_failed_item_and_preserves_positions() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![
        http(401, json!({"error":"denied"})),
        http(200, answer()),
    ])?;
    let run = Run::new(run_config(&root, &stub)?)?;
    let seen = Mutex::new(vec![]);
    let got = run.send_batch(&Call::default(), &vec![request(); 2], &|i, v| {
        if let Ok(mut seen) = seen.lock() {
            seen.push((i, v["ok"].clone()));
        }
    })?;
    assert_eq!(got[0]["index"], 0);
    assert_eq!(got[0]["result"]["error"]["id"], "http.auth");
    assert_eq!(got[1]["index"], 1);
    assert_eq!(got[1]["result"]["ok"], true);
    assert_eq!(
        seen.into_inner().map_err(|e| e.to_string())?,
        vec![(0, json!(false)), (1, json!(true))]
    );
    assert_eq!(stub.finish()?.len(), 2);
    assert_eq!(rows(&root)?.len(), 2);
    run.close()?;
    Ok(())
}
#[test]
fn req_sag_0702_two_workers_complete_before_either_callback_is_released() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let mut cfg = run_config(&root, &stub)?;
    cfg.workers = Some(2);
    let run = Run::new(cfg)?;
    let (arrivals, arrived) = mpsc::channel();
    let (release, permits) = mpsc::channel();
    let permits = Mutex::new(permits);
    let (result, workers) = std::thread::scope(
        |scope| -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
            let work = scope.spawn(|| {
                run.send_batch(&Call::default(), &vec![request(); 2], &|_, _| {
                    let _ = arrivals.send(std::thread::current().id());
                    if let Ok(rx) = permits.lock() {
                        let _ = rx.recv(); // The coordinator always supplies permits, including on its guard-error path.
                    }
                })
            });
            let workers = (0..2)
                .map(|_| arrived.recv_timeout(Duration::from_secs(5)))
                .collect::<Result<Vec<_>, _>>();
            for _ in 0..2 {
                release.send(())?;
            }
            let result = work.join().map_err(|_| "batch panicked")??;
            Ok((result, workers?))
        },
    )?;
    assert_ne!(workers.first(), workers.last());
    assert_eq!(result.as_array().ok_or("missing results")?.len(), 2);
    assert_eq!(stub.finish()?.len(), 2);
    run.close()?;
    Ok(())
}
#[test]
fn req_sag_0002_session_append_failure_keeps_provider_outcome_and_logs_once() -> Test {
    for status in [200, 401] {
        let root = Temp::new()?;
        fs::write(root.0.join("sessions"), "controlled directory obstruction")?;
        let body = if status == 200 {
            answer()
        } else {
            json!({"error":"provider message stays whole"})
        };
        let stub = Stub::new(vec![http(status, body.clone())])?;
        let run = Run::new(run_config(&root, &stub)?)?;
        let outcome = sc_agent::result(run.send(&Call::default(), &request()));
        if status == 200 {
            assert_eq!(outcome["value"]["answers"], body["answers"]);
        } else {
            assert_eq!(outcome["error"]["id"], "http.auth");
            assert_eq!(outcome["error"]["message"], body.to_string());
            outcome["error"]["session_id"]
                .as_str()
                .ok_or("no session")?
                .parse::<ulid::Ulid>()?;
        }
        assert_eq!(stub.finish()?.len(), 1);
        run.close()?;
        let text = fs::read_to_string(root.0.join("logs/decidr.log.jsonl"))?;
        let events = text
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            events
                .iter()
                .filter(|e| e["fields"]["id"] == "log.session_write")
                .count(),
            1
        );
    }
    Ok(())
}
#[test]
fn req_sag_0014_eight_mib_serialized_input_limit_precedes_network_and_batch_sends() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![])?;
    let run = Run::new(run_config(&root, &stub)?)?;
    let mut value = request();
    value["state"] = json!("");
    let exact = "x".repeat(8 * 1024 * 1024 - serde_json::to_vec(&value)?.len());
    value["state"] = json!(exact);
    assert_eq!(serde_json::to_vec(&value)?.len(), 8 * 1024 * 1024);
    sc_agent::validate_request_size(&value)?;
    value["state"] = json!(format!("{exact}x"));
    assert_eq!(serde_json::to_vec(&value)?.len(), 8 * 1024 * 1024 + 1);
    let got = sc_agent::result(run.send(&Call::default(), &value));
    assert_eq!(got["error"]["id"], "decision.request_too_large");
    let got = sc_agent::result(run.send_batch(&Call::default(), &[request(), value], &|_, _| {}));
    assert_eq!(got["error"]["id"], "decision.request_too_large");
    assert_eq!(rows(&root)?.len(), 1);
    assert!(stub.finish()?.is_empty());
    run.close()?;
    Ok(())
}
#[test]
fn run_snapshots_configuration_and_owns_or_borrows_one_logger() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let logger = Arc::new(logger(&root)?);
    let mut cfg = run_config(&root, &stub)?;
    cfg.logger = Some(logger.clone());
    let run = Run::new(cfg)?;
    run.send(&Call::default(), &request())?;
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"project":"changed"}).to_string(),
    )?;
    run.send(&Call::default(), &request())?;
    assert_eq!(rows(&root)?.len(), 2);
    assert!(rows(&root)?.iter().all(|r| r["project"] == "phase-b"));
    run.close()?;
    run.close()?;
    sc_agent::log(&logger, "after-close", json!({}))?;
    let failure = run
        .send(&Call::default(), &request())
        .err()
        .ok_or("closed run sent")?;
    assert_eq!(error_id(&failure), "decision.run_closed");
    assert_eq!(error_category(&failure), "validation");
    assert_eq!(rows(&root)?.len(), 2);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    let empty = Stub::new(vec![])?;
    let cfg = run_config(&root, &empty)?;
    let owned = Run::new(cfg)?;
    owned.close()?;
    owned.close()?;
    assert!(empty.finish()?.is_empty());
    Ok(())
}
#[test]
fn missing_project_is_a_call_error_and_cancellation_is_channel_driven() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![])?;
    fs::write(root.0.join("sc-agent.json"), "{}")?;
    let run = Run::new(run_config(&root, &stub)?)?;
    assert_eq!(
        error_id(
            &run.send(&Call::default(), &request())
                .err()
                .ok_or("missing project sent")?
        ),
        "config.missing_project"
    );
    run.close()?;
    assert!(stub.finish()?.is_empty());
    let cancel = Arc::new(Cancel::default());
    let (started, arrived) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (finished, result) = mpsc::channel();
    std::thread::scope(|scope| -> Test {
        let active = cancel.clone();
        let worker = scope.spawn(move || {
            let outcome = active.run(move || {
                let _ = started.send(());
                let _ = wait.recv();
            });
            let _ = finished.send(outcome);
        });
        let arrival = arrived.recv_timeout(Duration::from_secs(5));
        cancel.cancel();
        let outcome = result.recv_timeout(Duration::from_secs(5));
        // Cleanup precedes every assertion/error propagation: even a broken cancel implementation cannot strand the closure.
        let _ = release.send(());
        worker.join().map_err(|_| "cancel worker panicked")?;
        arrival?;
        assert_eq!(
            error_id(&outcome?.err().ok_or("cancelled closure succeeded")?),
            "decision.cancelled"
        );
        Ok(())
    })?;
    let flag = Arc::new(Cancel::default());
    let active = flag.clone();
    let stub = Stub::new(vec![Reply::Respond(Box::new(move |_| {
        active.cancel();
        Reply::Disconnect
    }))])?;
    let mut cfg = run_config(&root, &stub)?;
    cfg.project = Some("phase-b".into());
    cfg.cancel = Some(flag);
    let run = Run::new(cfg)?;
    assert_eq!(
        error_id(
            &run.send(&Call::default(), &request())
                .err()
                .ok_or("cancelled send succeeded")?
        ),
        "decision.cancelled"
    );
    assert_eq!(stub.finish()?.len(), 1);
    run.close()?;
    Ok(())
}

#[test]
fn req_sag_0007_confidence_is_not_rounded_or_cast() -> Test {
    let root = Temp::new()?;
    let response = json!({"model":"jev-1.14.0","answers":{"q":{"type":"choice","choice":"yes","confidence":0.875,"probabilities":{"yes":0.875,"no":0.125}}}});
    let input = json!({"state":"evidence","questions":{"q":{"type":"choice","criteria":{"yes":"true","no":"false"}}}});
    let stub = Stub::new(vec![http(200, response)])?;
    let run = Run::new(run_config(&root, &stub)?)?;
    assert_eq!(
        run.send(&Call::default(), &input)?["answers"]["q"]["confidence"],
        0.875
    );
    assert_eq!(stub.finish()?.len(), 1);
    run.close()?;
    Ok(())
}
