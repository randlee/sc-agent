pub mod support;
use sc_agent::{Cancel, Runner, batch, host_holdoff, send};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use support::*;

#[test]
fn req_sag_0201_0007_provider_wires_and_responses_preserve_values() -> Test {
    for (provider, model, path) in [
        ("typesafe", "jev", "/v1/systemone"),
        ("anthropic", "haiku", "/v1/messages"),
        ("openai", "luna", "/v1/chat/completions"),
        ("openrouter", "haiku", "/v1/chat/completions"),
        ("together", "luna", "/v1/chat/completions"),
        ("nous", "custom/model", "/v1/chat/completions"),
        ("local", "laya", "/v1/systemone"),
    ] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let raw = match provider {
            "anthropic" => json!({"content":[{"type":"text","text":answer().to_string()}]}),
            "typesafe" | "local" => answer(),
            _ => json!({"choices":[{"message":{"content":answer().to_string()}}]}),
        };
        let stub = Stub::new(vec![http(200, raw)])?;
        let mut opts = options(&root, &stub.url);
        opts.model = model.into();
        opts.provider = Some(provider.into());
        opts.environment.extend([
            ("OPENROUTER_API_KEY".into(), "test-secret".into()),
            ("TOGETHER_API_KEY".into(), "test-secret".into()),
            ("NOUS_API_KEY".into(), "test-secret".into()),
        ]);
        if provider == "local" {
            opts.settings.laya = Some(Runner {
                host: "127.0.0.1".into(),
                port: 1,
                model: "configured-local".into(),
            });
        }
        let value = send(
            &request(),
            &opts,
            &logger,
            &Cancel::default(),
            &no_wait,
            &keyring,
        )?;
        assert_eq!(value["answers"], answer()["answers"]);
        assert_eq!(value["answers"]["q"]["noul"], 0.123456789);
        let seen = stub.finish()?;
        let first = seen.first().ok_or("request missing")?;
        assert_eq!(first.path, path);
        if ["typesafe", "local"].contains(&provider) {
            let mut expected = request();
            expected["model"] = json!(if provider == "local" {
                "configured-local".to_owned()
            } else {
                sc_agent::model(model, Some(provider))?.1
            });
            assert_eq!(first.body, expected);
        } else {
            let schema = if provider == "anthropic" {
                &first.body["output_config"]["format"]["schema"]
            } else {
                &first.body["response_format"]["json_schema"]["schema"]
            };
            assert_eq!(schema["required"], json!(["answers"]));
            assert_eq!(schema["properties"]["answers"]["required"], json!(["q"]));
            assert_eq!(
                schema["properties"]["answers"]["properties"]["q"]["properties"]["type"]["enum"],
                json!(["noul"])
            );
            assert!(first.body["messages"].to_string().contains("evidence"));
        }
        if provider == "anthropic" {
            assert!(
                first
                    .headers
                    .iter()
                    .any(|(k, v)| k == "x-api-key" && v == "test-secret")
            );
        } else if provider != "local" {
            assert!(
                first
                    .headers
                    .iter()
                    .any(|(k, v)| k == "authorization" && v == "Bearer test-secret")
            );
        }
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn req_sag_0702_timeout_is_per_call_and_uses_the_transport() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let replies = vec![
        Reply::Never,
        Reply::Never,
        Reply::Never,
        Reply::Never,
        Reply::Never,
    ];
    let stub = Stub::new(replies)?;
    let configured_timeout = options(&root, &stub.url).timeout;
    let mut overridden = options(&root, &stub.url);
    overridden.timeout = Duration::from_millis(50);

    let signal = stub.request_signal();
    let timed_out = std::thread::scope(|scope| {
        let logger_ref = &logger;
        let overridden_ref = &overridden;
        let client = scope.spawn(move || {
            let outcome = send(
                &request(),
                overridden_ref,
                logger_ref,
                &Cancel::default(),
                &no_wait,
                &keyring,
            );
            let _ = signal.send(());
            outcome
        });
        stub.wait_for_request()?;
        let outcome = client.join().map_err(|_| "send thread panicked")?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(sc_agent::report(&logger, outcome))
    })?;
    assert_eq!(timed_out["error"]["id"], "http.network");
    assert_eq!(timed_out["error"]["attempts"], 5);
    assert_eq!(configured_timeout, Duration::from_secs(5));

    let success_stub = Stub::new(vec![http(200, answer())])?;
    let configured = options(&root, &success_stub.url);
    assert_eq!(configured.timeout, configured_timeout);
    let plain = send(
        &request(),
        &configured,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    )?;
    assert_eq!(plain["answers"], answer()["answers"]);
    let _ = stub.finish()?;
    assert_eq!(success_stub.finish()?.len(), 1);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0601_busy_resends_unchanged_retry_after_and_exponential_backoff() -> Test {
    for status in [429, 502, 529] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let replies = vec![
            Reply::Http(status, vec![("Retry-After", "1.1")], "busy".into()),
            http(status, json!({})),
            Reply::Http(status, vec![("Retry-After", "malformed")], "busy".into()),
            http(200, answer()),
        ];
        let stub = Stub::new(replies)?;
        let opts = options(&root, &stub.url);
        let waits = Mutex::new(vec![]);
        let wait = |delay| {
            if let Ok(mut seen) = waits.lock() {
                seen.push(delay);
            }
        };
        let got = send(
            &request(),
            &opts,
            &logger,
            &Cancel::default(),
            &wait,
            &keyring,
        )?;
        assert_eq!(got["attempts"], 4);
        let waits = waits.into_inner().map_err(|_| "wait recorder poisoned")?;
        assert_eq!(waits.first(), Some(&Duration::from_secs_f64(1.1)));
        assert_eq!(waits.get(1), Some(&Duration::from_secs(2)));
        assert_eq!(waits.get(2), Some(&Duration::from_secs(4)));
        let seen = stub.finish()?;
        assert_eq!(seen.len(), 4);
        for sent in &seen {
            assert_eq!(sent.body, seen.first().ok_or("no sends")?.body);
        }
        assert_eq!(rows(&root)?.len(), 1); // All resends belong to one session.
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn req_sag_0601_0017_busy_and_network_exhaustion_report_five_sends() -> Test {
    for network in [false, true] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
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
        let waits = Mutex::new(vec![]);
        let got = sc_agent::result(send(
            &request(),
            &options(&root, &stub.url),
            &logger,
            &Cancel::default(),
            &|delay| {
                if let Ok(mut w) = waits.lock() {
                    w.push(delay);
                }
            },
            &keyring,
        ));
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
            waits.into_inner().map_err(|_| "wait recorder poisoned")?,
            vec![1, 2, 4, 8]
                .into_iter()
                .map(Duration::from_secs)
                .collect::<Vec<_>>()
        );
        logger.shutdown()?;
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
        let logger = logger(&root)?;
        let stub = Stub::new(vec![Reply::Http(status, vec![], body.into())])?;
        let got = sc_agent::report(
            &logger,
            send(
                &request(),
                &options(&root, &stub.url),
                &logger,
                &Cancel::default(),
                &no_wait,
                &keyring,
            ),
        );
        assert_eq!(got["error"]["id"], id);
        assert_eq!(got["error"]["category"], category);
        assert_eq!(
            got["error"]["message"],
            body.replace("test-secret", "[redacted]")
        );
        assert_eq!(got["error"]["attempts"], 1);
        assert_eq!(stub.finish()?.len(), 1);
        logger.flush()?;
        let general = std::fs::read_to_string(root.0.join("logs/rust-test.log.jsonl"))?;
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
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn req_sag_0601_holdoff_lock_is_shared_only_by_host_and_held_during_wait() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(429, json!({})), http(200, answer())])?;
    let host = host_holdoff("127.0.0.1")?;
    assert!(Arc::ptr_eq(&host, &host_holdoff("127.0.0.1")?));
    let other = host_holdoff("independent.invalid")?;
    assert!(!Arc::ptr_eq(&host, &other));
    let observations = Mutex::new(vec![]);
    send(
        &request(),
        &options(&root, &stub.url),
        &logger,
        &Cancel::default(),
        &|_| {
            let blocked = matches!(host.try_lock(), Err(std::sync::TryLockError::WouldBlock));
            let independent = other.try_lock().is_ok();
            if let Ok(mut seen) = observations.lock() {
                seen.push((blocked, independent));
            }
        },
        &keyring,
    )?;
    assert_eq!(
        observations
            .into_inner()
            .map_err(|_| "holdoff recorder poisoned")?,
        vec![(true, true)]
    );
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0010_0011_cancel_during_retry_keeps_every_batch_position_and_row() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![Reply::Http(
        429,
        vec![("Retry-After", "600")],
        "busy".into(),
    )])?;
    let cancel = Cancel::default();
    let waits = Mutex::new(vec![]);
    let completed = Mutex::new(vec![]);
    let got = batch(
        &vec![request(); 3],
        &options(&root, &stub.url),
        &logger,
        &cancel,
        &|delay| {
            if let Ok(mut seen) = waits.lock() {
                seen.push(delay);
            }
            cancel.cancel();
            cancel.wait(Duration::from_secs(600)); // Already signalled: no real wait, regardless of scheduling.
        },
        &keyring,
        &|index, value| {
            if let Ok(mut seen) = completed.lock() {
                seen.push((index, value.clone()));
            }
        },
    )?;
    assert!(cancel.is_cancelled());
    assert_eq!(stub.finish()?.len(), 1);
    let results = got.as_array().ok_or("batch results missing")?;
    assert_eq!(results.len(), 3);
    for (index, result) in results.iter().enumerate() {
        assert_eq!(
            result["result"]["error"]["attempts"],
            if index == 0 { 1 } else { 0 }
        );
        let id = result["result"]["error"]["session_id"]
            .as_str()
            .ok_or("cancelled result has no session id")?;
        id.parse::<ulid::Ulid>()?;
        assert_eq!(result["index"], index);
        assert_eq!(result["result"]["error"]["id"], "decision.cancelled");
    }
    assert_eq!(
        completed
            .into_inner()
            .map_err(|_| "completion recorder poisoned")?
            .len(),
        3
    );
    assert_eq!(
        waits.into_inner().map_err(|_| "wait recorder poisoned")?,
        vec![Duration::from_secs(600)]
    );
    assert_eq!(rows(&root)?.len(), 3);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0011_precancel_never_reads_a_key_and_invalid_tags_write_no_row() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let mut opts = options(&root, "http://127.0.0.1:1/v1");
    let cancel = Cancel::default();
    cancel.cancel();
    opts.environment.clear();
    let keys = std::sync::atomic::AtomicUsize::new(0);
    let got = sc_agent::result(send(&request(), &opts, &logger, &cancel, &no_wait, &|_| {
        keys.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err("unexpected lookup".into())
    }));
    assert_eq!(got["error"]["id"], "decision.cancelled");
    assert_eq!(keys.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(rows(&root)?.len(), 1);
    // REQ-SAG-0004: invalid/missing project or invalid role is refused without creating a session.
    for invalid_role in [false, true] {
        let isolated = Temp::new()?;
        let mut bad = options(&isolated, "http://127.0.0.1:1/v1");
        if invalid_role {
            bad.agent_role = "Phase B".into();
        } else {
            bad.settings.project = Some("Phase B".into());
        }
        let error = send(
            &request(),
            &bad,
            &logger,
            &Cancel::default(),
            &no_wait,
            &keyring,
        )
        .err()
        .ok_or("invalid tag accepted")?;
        assert_eq!(serde_json::to_value(error)?["id"], "log.invalid_slug");
        assert!(!isolated.0.join("sessions").exists());
    }
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0702_batch_continues_after_failed_item_and_preserves_positions() -> Test {
    // ADR-SAG-0001: one failed request does not stop the stream.
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        http(401, json!({"error":"denied"})),
        http(200, answer()),
    ])?;
    let seen = Mutex::new(vec![]);
    let got = batch(
        &vec![request(); 2],
        &options(&root, &stub.url),
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
        &|index, value| {
            if let Ok(mut items) = seen.lock() {
                items.push((index, value["ok"].clone()));
            }
        },
    )?;
    assert_eq!(got[0]["index"], 0);
    assert_eq!(got[0]["result"]["error"]["id"], "http.auth");
    assert_eq!(got[1]["index"], 1);
    assert_eq!(got[1]["result"]["ok"], true);
    assert_eq!(
        seen.into_inner()
            .map_err(|_| "callback recorder poisoned")?,
        vec![(0, json!(false)), (1, json!(true))]
    );
    assert_eq!(stub.finish()?.len(), 2);
    assert_eq!(rows(&root)?.len(), 2);
    let mut invalid = options(&root, "http://127.0.0.1:1/v1");
    invalid.settings.workers = 0;
    assert_eq!(
        sc_agent::result(batch(
            &[],
            &invalid,
            &logger,
            &Cancel::default(),
            &no_wait,
            &keyring,
            &|_, _| {}
        ))["error"]["id"],
        "decidr.invalid_workers"
    );
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0702_two_workers_reach_controlled_keyring_before_either_is_released() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let mut opts = options(&root, &stub.url);
    opts.environment.clear();
    opts.settings.workers = 2;
    let (arrivals, arrived) = mpsc::channel();
    let (release, permits) = mpsc::channel();
    let permits = Mutex::new(permits);
    let keys = |_: &str| {
        arrivals
            .send(std::thread::current().id())
            .map_err(|e| e.to_string())?;
        permits
            .lock()
            .map_err(|e| e.to_string())?
            .recv_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        Ok("test-secret".into())
    };
    let (result, workers) = std::thread::scope(
        |scope| -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
            let work = scope.spawn(|| {
                batch(
                    &vec![request(); 2],
                    &opts,
                    &logger,
                    &Cancel::default(),
                    &no_wait,
                    &keys,
                    &|_, _| {},
                )
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
    assert_eq!(result.as_array().ok_or("missing batch results")?.len(), 2);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0002_session_append_failure_keeps_provider_outcome_and_logs_once() -> Test {
    for status in [200, 401] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        std::fs::write(root.0.join("sessions"), "controlled directory obstruction")?;
        let body = if status == 200 {
            answer()
        } else {
            json!({"error":"provider message stays whole"})
        };
        let stub = Stub::new(vec![http(status, body.clone())])?;
        let outcome = sc_agent::result(send(
            &request(),
            &options(&root, &stub.url),
            &logger,
            &Cancel::default(),
            &no_wait,
            &keyring,
        ));
        if status == 200 {
            assert_eq!(outcome["value"]["answers"], body["answers"]);
        } else {
            assert_eq!(outcome["error"]["id"], "http.auth");
            assert_eq!(outcome["error"]["message"], body.to_string());
            outcome["error"]["session_id"]
                .as_str()
                .ok_or("failed request lacks ULID")?
                .parse::<ulid::Ulid>()?;
        }
        assert_eq!(stub.finish()?.len(), 1);
        logger.flush()?;
        let text = std::fs::read_to_string(root.0.join("logs/rust-test.log.jsonl"))?;
        let events = text
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event["fields"]["id"] == "log.session_write")
                .count(),
            1
        );
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn req_sag_0014_eight_mib_serialized_input_limit_precedes_network_and_batch_sends() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let mut value = request();
    value["state"] = json!("");
    let exact = "x".repeat(8 * 1024 * 1024 - serde_json::to_vec(&value)?.len());
    value["state"] = json!(exact);
    assert_eq!(serde_json::to_vec(&value)?.len(), 8 * 1024 * 1024);
    sc_agent::validate_request_size(&value)?;
    value["state"] = json!(format!("{exact}x"));
    assert_eq!(serde_json::to_vec(&value)?.len(), 8 * 1024 * 1024 + 1);
    let opts = options(&root, "http://127.0.0.1:1/v1");
    let got = sc_agent::result(send(
        &value,
        &opts,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    ));
    assert_eq!(got["error"]["id"], "decision.request_too_large");
    let got = sc_agent::result(batch(
        &[request(), value],
        &opts,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
        &|_, _| {},
    ));
    assert_eq!(got["error"]["id"], "decision.request_too_large");
    assert_eq!(rows(&root)?.len(), 1); // Batch refused all input before its first request.
    logger.shutdown()?;
    Ok(())
}
