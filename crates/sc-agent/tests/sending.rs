pub mod support;
use sc_agent::{Cancel, Runner};
use serde_json::{Value, json};
use std::{
    sync::{Mutex, mpsc},
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
        ("laya", "laya", "/v1/systemone"),
    ] {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let mut raw = match provider {
            "anthropic" => json!({"content":[{"type":"text","text":answer().to_string()}]}),
            "typesafe" | "laya" => answer(),
            _ => json!({"choices":[{"message":{"content":answer().to_string()}}]}),
        };
        raw["model"] = json!("actual-model");
        raw["usage"] = json!({"input_tokens": 7});
        let stub = Stub::new(vec![http(200, raw)])?;
        let mut opts = options(&root, &stub.url);
        opts.model = model.into();
        opts.settings.models.insert(
            opts.model.clone(),
            sc_agent::ModelEntry {
                released: None,
                provider: provider.into(),
                id: pinned_model(provider, &opts.model)?.1,
            },
        );
        opts.environment.extend([
            ("OPENROUTER_API_KEY".into(), "test-secret".into()),
            ("TOGETHER_API_KEY".into(), "test-secret".into()),
            ("NOUS_API_KEY".into(), "test-secret".into()),
        ]);
        if provider == "laya" {
            opts.settings.local.insert(
                "laya".into(),
                Runner {
                    host: "127.0.0.1".into(),
                    port: 1,
                },
            );
        }
        let value = {
            let run = run(opts.clone(), &logger)?;

            run.send(&request(), Some(false))
        }?;
        assert_eq!(value["answers"], answer()["answers"]);
        assert_eq!(value["answers"]["q"]["noul"], 0.123456789);
        let seen = stub.finish()?;
        let first = seen.first().ok_or("request missing")?;
        assert_eq!(first.path, path);
        let recorded = rows(&root)?;
        let row = recorded.first().ok_or("session row missing")?;
        assert_eq!(row["schema"], "sc-agent-session/2");
        assert_eq!(row["provider"], provider);
        assert_eq!(row["model_requested"], pinned_model(provider, model)?.1);
        assert_eq!(row["model_answered"], "actual-model");
        assert_eq!(row["usage"], json!({"input_tokens":7}));
        assert_eq!(
            row["request_sha256"],
            sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
                utf8_file_bytes: &serde_json::to_vec(&first.body)?
            })?
            .template()
            .to_string()
        );
        if ["typesafe", "laya"].contains(&provider) {
            let mut expected = request();
            expected["model"] = json!(pinned_model(provider, model)?.1);
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
        } else if provider != "laya" {
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
        let waits = std::sync::Arc::new(Mutex::new(vec![]));
        let observed = waits.clone();
        let wait = move |delay| {
            if let Ok(mut seen) = observed.lock() {
                seen.push(delay);
            }
            Ok(())
        };
        let got = {
            let mut run = run(opts.clone(), &logger)?;

            run.wait = Box::new(wait);

            run.send(&request(), Some(false))
        }?;
        assert_eq!(got["attempts"], 4);
        let waits = waits
            .lock()
            .map(|v| v.clone())
            .map_err(|_| "wait recorder poisoned")?;
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
        let waits = std::sync::Arc::new(Mutex::new(vec![]));
        let got = sc_agent::result({
            let mut run = run(options(&root, &stub.url), &logger)?;

            let waits = waits.clone();
            let _cancel = run.cancel.clone();
            run.wait = Box::new(move |delay| {
                if let Ok(mut w) = waits.lock() {
                    w.push(delay);
                }
                Ok(())
            });

            run.send(&request(), Some(false))
        });
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
            waits
                .lock()
                .map(|v| v.clone())
                .map_err(|_| "wait recorder poisoned")?,
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
        let got = sc_agent::report(&logger, {
            let run = run(options(&root, &stub.url), &logger)?;

            run.send(&request(), Some(false))
        });
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
        assert!(!root.0.join("sessions").exists());
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
fn invalid_utf8_provider_error_keeps_status_and_surrounding_text() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let replies = (0..5)
        .map(|_| {
            Reply::Raw(
                401,
                vec![("Content-Type", "text/plain")],
                b"before\xffafter".to_vec(),
            )
        })
        .collect();
    let stub = Stub::new(replies)?;
    let failure = sc_agent::result(run(options(&root, &stub.url), &logger)?.send(&request(), None));
    let requests = stub.finish()?;
    assert_eq!(failure["error"]["id"], "http.auth");
    assert_eq!(failure["error"]["message"], "before?after");
    assert_eq!(failure["error"]["attempts"], 1);
    assert_eq!(requests.len(), 1);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0002_provider_error_over_ten_mib_is_preserved() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let detail = "x".repeat(10 * 1024 * 1024 + 1);
    let stub = Stub::new(vec![http(401, json!({"error":detail}))])?;
    let mut opts = options(&root, &stub.url);
    // Failure-only hang guard, not a throughput target; the response content decides success.
    // Keep the first transport failure rather than retrying an exhausted one-reply stub.
    opts.timeout = Duration::from_secs(600);
    opts.settings.max_sends = 1;
    let failure = sc_agent::result(run(opts, &logger)?.send(&request(), None));
    let stub_outcome = stub.finish();
    let error = &failure["error"];
    let message = error["message"].as_str().unwrap_or_default();
    let diagnostic = format!(
        "id={}, attempts={}, message_bytes={}, prefix={:?}, stub={:?}",
        error["id"],
        error["attempts"],
        message.len(),
        message.chars().take(160).collect::<String>(),
        stub_outcome,
    );
    assert_eq!(error["id"], "http.auth", "{diagnostic}");
    assert!(message.contains(&detail), "{diagnostic}");
    assert_eq!(stub_outcome?.len(), 1);
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
    let cancel = std::sync::Arc::new(Cancel::default());
    let waits = std::sync::Arc::new(Mutex::new(vec![]));
    let completed = Mutex::new(vec![]);
    let got = {
        let mut run = run(options(&root, &stub.url), &logger)?;
        run.cancel = cancel.clone();
        let waits = waits.clone();
        let cancel = run.cancel.clone();
        run.wait = Box::new(move |delay| {
            if let Ok(mut seen) = waits.lock() {
                seen.push(delay);
            }
            cancel.cancel();
            cancel.wait(Duration::from_secs(600)) // Already signalled: no real wait, regardless of scheduling.
        });

        run.send_batch(&vec![request(); 3], Some(false), &|index, value| {
            if let Ok(mut seen) = completed.lock() {
                seen.push((index, value.clone()));
            }
        })
    }?;
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
        waits
            .lock()
            .map(|v| v.clone())
            .map_err(|_| "wait recorder poisoned")?,
        vec![Duration::from_secs(600)]
    );
    assert!(!root.0.join("sessions").exists());
    logger.shutdown()?;
    Ok(())
}

#[test]
fn req_sag_0011_precancel_never_reads_a_key_and_invalid_tags_write_no_row() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let mut opts = options(&root, "http://127.0.0.1:1/v1");
    let cancel = std::sync::Arc::new(Cancel::default());
    cancel.cancel();
    opts.environment.clear();
    let keys = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let got = sc_agent::result({
        let mut run = run(opts.clone(), &logger)?;
        run.cancel = cancel.clone();

        let keys = keys.clone();
        run.keyring = Box::new(move |_| {
            keys.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(sc_agent::KeyringError::Read("unexpected lookup".into()))
        });
        run.send(&request(), Some(false))
    });
    assert_eq!(got["error"]["id"], "decision.cancelled");
    assert_eq!(keys.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!root.0.join("sessions").exists());
    // REQ-SAG-0004: invalid/missing project or invalid role is refused without creating a session.
    for invalid_role in [false, true] {
        let isolated = Temp::new()?;
        let mut bad = options(&isolated, "http://127.0.0.1:1/v1");
        if invalid_role {
            bad.agent_role = "Phase B".into();
        } else {
            bad.settings.project = Some("Phase B".into());
        }
        let error = {
            let run = run(bad.clone(), &logger)?;

            run.send(&request(), Some(false))
        }
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
    let got = {
        let run = run(options(&root, &stub.url), &logger)?;

        run.send_batch(&vec![request(); 2], Some(false), &|index, value| {
            if let Ok(mut items) = seen.lock() {
                items.push((index, value["ok"].clone()));
            }
        })
    }?;
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
    let recorded = rows(&root)?;
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded.first().ok_or("success row missing")?["result"]["ok"],
        true
    );
    let mut invalid = options(&root, "http://127.0.0.1:1/v1");
    invalid.settings.workers = 0;
    assert_eq!(
        sc_agent::result({
            let run = run(invalid.clone(), &logger)?;

            run.send_batch(&[], Some(false), &|_, _| {})
        })["error"]["id"],
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
    let permit_error = std::sync::Arc::new(Mutex::new(None));
    let recorded_error = permit_error.clone();
    let keys = move |_: &str| {
        arrivals
            .send(std::thread::current().id())
            .map_err(|e| e.to_string())?;
        if let Err(error) = permits.lock().map_err(|e| e.to_string())?.recv() {
            *recorded_error.lock().map_err(|e| e.to_string())? =
                Some(format!("missing keyring release permit: {error}"));
            return Err("platform keyring is unavailable".to_owned().into());
        }
        Ok("test-secret".into())
    };
    let (result, workers) = std::thread::scope(
        |scope| -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
            let work = scope.spawn(|| {
                let mut run = run(opts.clone(), &logger)?;

                run.keyring = Box::new(keys);
                run.send_batch(&vec![request(); 2], Some(false), &|_, _| {})
            });
            let workers = (0..2)
                .map(|_| arrived.recv_timeout(Duration::from_secs(5)))
                .collect::<Result<Vec<_>, _>>();
            if workers.is_ok() {
                for _ in 0..2 {
                    release.send(())?;
                }
            }
            drop(release);
            let result = work.join().map_err(|_| "batch panicked")??;
            Ok((result, workers))
        },
    )?;
    let permit_error = permit_error
        .lock()
        .map(|value| value.clone())
        .map_err(|_| "keyring permit error recorder poisoned")?;
    let workers = workers.map_err(|error| {
        std::io::Error::other(format!(
            "worker arrival deadline expired: {error}; permit_error={permit_error:?}; batch={result:?}"
        ))
    })?;
    assert_eq!(permit_error, None, "keyring permit failed");
    assert_ne!(workers.first(), workers.last());
    assert_eq!(result.as_array().ok_or("missing batch results")?.len(), 2);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_session_write_failure() -> Test {
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
        let outcome = sc_agent::report(&logger, {
            let run = run(options(&root, &stub.url), &logger)?;

            run.send(&request(), Some(false))
        });
        if status == 200 {
            assert_eq!(outcome["error"]["id"], "log.session_write");
        } else {
            assert_eq!(outcome["error"]["id"], "http.auth");
            assert_eq!(outcome["error"]["message"], body.to_string());
        }
        outcome["error"]["session_id"]
            .as_str()
            .ok_or("failed request lacks ULID")?
            .parse::<ulid::Ulid>()?;
        assert!(
            !root
                .0
                .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/runs.jsonl")
                .exists()
        );
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
            usize::from(status == 200)
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event["fields"]["id"] == "http.auth")
                .count(),
            usize::from(status == 401)
        );
        logger.shutdown()?;
    }
    Ok(())
}

#[test]
fn cancel_after_a_lower_layer_error_keeps_it() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let mut opts = options(&root, "http://127.0.0.1:1/v1");
    opts.environment.clear();
    let cancel = std::sync::Arc::new(Cancel::default());
    let failure = sc_agent::result({
        let mut run = run(opts.clone(), &logger)?;
        run.cancel = cancel.clone();

        let cancel = run.cancel.clone();
        run.keyring = Box::new(move |_| {
            cancel.cancel();
            Err(sc_agent::KeyringError::Read("controlled failure".into()))
        });
        run.send(&request(), Some(false))
    });
    assert_eq!(failure["error"]["id"], "secrets.keyring_read");
    assert!(cancel.is_cancelled());
    logger.shutdown()?;
    Ok(())
}
