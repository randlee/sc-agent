pub mod support;
use sc_agent::{
    AnthropicCompat, Cancel, DEFAULT_SYSTEM_PROMPT, Decision, Exchange, Jev, Llm, LlmRequest,
    OpenAiCompat, Options, Route, Runner, result,
};
use serde_json::{Value, json};
use support::*;

fn header<'a>(seen: &'a support::Request, name: &str) -> Option<&'a str> {
    seen.headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}
fn llm_request() -> LlmRequest {
    LlmRequest {
        system: Some("S".into()),
        messages: json!([{"role":"user","content":"hi"}]),
        schema: Some(json!({"type":"object"})),
    }
}
fn with_route<T>(
    options: &Options,
    (provider, name): (&str, &str),
    base: &str,
    run: impl Fn(&Route<'_>) -> T,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
    let (provider, model) = pinned_model(provider, name)?;
    let exchange = Exchange::default();
    let route = Route {
        options,
        provider,
        model,
        base: base.into(),
        cancel: &Cancel::default(),
        wait: &no_wait,
        keyring: &keyring,
        exchange: &exchange,
    };
    Ok(run(&route))
}

#[test]
fn jev_decide_posts_the_request_with_the_resolved_model() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("typesafe", "jev"), &stub.url, |route| {
        Jev(route).decide(&request())
    })??;
    assert_eq!(reply, answer());
    let seen = stub.finish()?;
    let mut expected = request();
    expected["model"] = json!("jev-latest");
    assert_eq!(seen[0].path, "/v1/systemone");
    assert_eq!(seen[0].body, expected);
    Ok(())
}

#[test]
fn non_decision_llm_selected_by_short_name_returns_the_json_the_model_wrote() -> Test {
    let root = Temp::new()?;
    let raw = json!({"choices":[{"message":{"content":"{\"a\":1}"}}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("openai", "sol"), &stub.url, |route| {
        OpenAiCompat(route).call(&llm_request())
    })??;
    assert_eq!(reply, json!({"a":1}));
    let seen = stub.finish()?;
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].body["model"], pinned_model("openai", "sol")?.1);
    assert_eq!(seen[0].path, "/v1/chat/completions");
    assert_eq!(
        seen[0].body["messages"],
        json!([{"role":"system","content":"S"},{"role":"user","content":"hi"}])
    );
    assert_eq!(
        seen[0].body["response_format"]["json_schema"]["schema"],
        json!({"type":"object"})
    );
    assert_eq!(
        header(&seen[0], "authorization"),
        Some("Bearer test-secret")
    );
    Ok(())
}

#[test]
fn anthropic_compat_call_returns_the_json_the_model_wrote() -> Test {
    let root = Temp::new()?;
    let raw = json!({"content":[{"type":"text","text":"{\"a\":1}"}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("anthropic", "haiku"), &stub.url, |route| {
        AnthropicCompat(route).call(&llm_request())
    })??;
    assert_eq!(reply, json!({"a":1}));
    let seen = stub.finish()?;
    assert_eq!(seen[0].path, "/v1/messages");
    assert_eq!(seen[0].body["system"], "S");
    assert_eq!(seen[0].body["max_tokens"], 128000);
    assert_eq!(
        seen[0].body["messages"],
        json!([{"role":"user","content":"hi"}])
    );
    assert_eq!(
        seen[0].body["output_config"]["format"]["schema"],
        json!({"type":"object"})
    );
    assert_eq!(header(&seen[0], "x-api-key"), Some("test-secret"));
    assert_eq!(header(&seen[0], "anthropic-version"), Some("2023-06-01"));
    assert_eq!(header(&seen[0], "authorization"), None);
    Ok(())
}

#[test]
fn luna_and_haiku_send_the_default_then_the_configured_system_prompt() -> Test {
    for (model, text) in [
        ("luna", "luna-system-prompt"),
        ("haiku", "haiku-system-prompt"),
    ] {
        for configured in [None, Some("X")] {
            let root = Temp::new()?;
            let logger = logger(&root)?;
            let content = answer().to_string();
            let raw = if model == "luna" {
                json!({"choices":[{"message":{"content":content}}]})
            } else {
                json!({"content":[{"type":"text","text":content}]})
            };
            let stub = Stub::new(vec![http(200, raw)])?;
            let mut opts = options(&root, &stub.url);
            opts.model = model.into();
            if let Some(prompt) = configured {
                opts.settings.decidr.insert(text.into(), prompt.into());
            }
            let value = {
                let run = run(opts.clone(), &logger)?;

                run.send(&request(), Some(false))
            }?;
            assert_eq!(value["answers"], answer()["answers"]);
            let body = &stub.finish()?[0].body;
            let system = if model == "luna" {
                body["messages"][0]["content"].clone()
            } else {
                body["system"].clone()
            };
            assert_eq!(system, json!(configured.unwrap_or(DEFAULT_SYSTEM_PROMPT)));
            logger.shutdown()?;
        }
    }
    Ok(())
}

#[test]
fn decide_over_an_llm_rejects_an_answer_of_the_wrong_type() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let wrong = json!({"answers":{"q":{"type":"choice","choice":"a","confidence":1.0,"probabilities":{"a":1.0}}}});
    let raw = json!({"choices":[{"message":{"content":wrong.to_string()}}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let mut opts = options(&root, &stub.url);
    opts.model = "luna".into();
    let failure = result({
        let run = run(opts.clone(), &logger)?;

        run.send(&request(), Some(false))
    });
    assert_eq!(failure["error"]["id"], "decision.schema_mismatch");
    let _ = stub.finish()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn max_sends_setting_limits_the_sends_of_one_request() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let busy = || http(502, json!({"error":"busy"}));
    let stub = Stub::new(vec![busy(), busy()])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 2;
    let failure = result({
        let run = run(opts.clone(), &logger)?;

        run.send(&request(), Some(false))
    });
    assert_eq!(failure["error"]["id"], "http.provider_busy");
    assert_eq!(failure["error"]["attempts"], 2);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    let _: Value = failure;
    Ok(())
}

#[test]
fn compat_call_reports_response_parse_for_text_that_is_not_json_or_missing() -> Test {
    for (reply, expected) in [
        (
            json!({"choices":[{"message":{"content":"not json"}}]}),
            "not json",
        ),
        (json!({"choices":[]}), "provider reply has no text"),
    ] {
        let root = Temp::new()?;
        let stub = Stub::new(vec![http(200, reply)])?;
        let opts = options(&root, &stub.url);
        let failure = with_route(&opts, ("openai", "luna"), &stub.url, |route| {
            OpenAiCompat(route).call(&llm_request())
        })?;
        let failure = result(failure);
        assert_eq!(failure["error"]["id"], "decision.response_parse");
        let message = failure["error"]["message"].as_str().ok_or("no message")?;
        assert!(message.contains(expected), "{message}");
        let _ = stub.finish()?;
    }
    Ok(())
}

fn runner(stub: &Stub) -> Result<Runner, Box<dyn std::error::Error + Send + Sync>> {
    let address = stub
        .url
        .trim_start_matches("http://")
        .trim_end_matches("/v1");
    let (host, port) = address.split_once(':').ok_or("stub address")?;
    Ok(Runner {
        host: host.into(),
        port: port.parse()?,
    })
}
fn sent(root: &Temp, opts: &Options) -> Result<Value, sc_agent::Error> {
    let logger = logger(root).map_err(|e| sc_agent::error("config.logger", e))?;
    let reply = {
        let run = run(opts.clone(), &logger)?;

        run.send(&request(), Some(false))
    };
    logger
        .shutdown()
        .map_err(|e| sc_agent::error("config.logger", e))?;
    reply
}

#[test]
fn local_endpoints() -> Test {
    for (model, name, id) in [
        ("decider", "decider_4b", "decider-4b-v2.1"),
        ("decider-2b-v11", "decider_2b", "decider-2b-v11"),
        ("laya", "laya", "laya:en"),
    ] {
        let root = Temp::new()?;
        let stub = Stub::new(vec![http(200, answer())])?;
        let mut opts = options(&root, "");
        opts.base_url = None;
        opts.model = model.into();
        opts.settings.local.insert(name.into(), runner(&stub)?);
        sent(&root, &opts)?;
        let seen = stub.finish()?;
        assert_eq!(seen[0].path, "/v1/systemone");
        assert_eq!(seen[0].body["model"], id);
        for secret in ["authorization", "x-api-key"] {
            assert_eq!(header(&seen[0], secret), None);
        }
    }
    let root = Temp::new()?;
    let mut opts = options(&root, "");
    opts.model = "decider-2b-v11".into();
    opts.settings.local.insert(
        "decider_4b".into(),
        Runner {
            host: "h".into(),
            port: 1,
        },
    );
    let failure = sent(&root, &opts).err().ok_or("unset runner sent")?;
    let failure = serde_json::to_value(failure)?;
    assert_eq!(failure["id"], "config.missing_setting");
    assert!(
        failure["message"]
            .as_str()
            .is_some_and(|m| m.contains("decider_2b.host"))
    );
    Ok(())
}

#[test]
fn keyless_resolution() -> Test {
    let anthropic = json!({"content":[{"type":"text","text":answer().to_string()}]});
    let chat = json!({"choices":[{"message":{"content":answer().to_string()}}]});
    for (model, keys, path, id) in [
        (
            "haiku",
            &["ANTHROPIC_API_KEY"][..],
            "/v1/messages",
            "claude-haiku-5-5",
        ),
        (
            "haiku",
            &["NOUS_API_KEY"],
            "/v1/chat/completions",
            "anthropic/claude-haiku-5.5",
        ),
        (
            "luna",
            &["NOUS_API_KEY"],
            "/v1/chat/completions",
            "openai/gpt-6-luna",
        ),
        (
            "haiku",
            &["OPENROUTER_API_KEY"],
            "/v1/chat/completions",
            "anthropic/claude-haiku-5.5",
        ),
        (
            "haiku",
            &["TOGETHER_API_KEY", "NOUS_API_KEY"],
            "/v1/chat/completions",
            "anthropic/claude-haiku-5-5",
        ),
    ] {
        let root = Temp::new()?;
        let stub = Stub::new(vec![http(
            200,
            if path == "/v1/messages" {
                anthropic.clone()
            } else {
                chat.clone()
            },
        )])?;
        let mut opts = options(&root, &stub.url);
        opts.environment = keys.iter().map(|k| ((*k).into(), "held".into())).collect();
        opts.model = model.into();
        sent(&root, &opts)?;
        let seen = stub.finish()?;
        assert_eq!(
            (seen[0].path.as_str(), &seen[0].body["model"]),
            (path, &json!(id)),
            "{model} {keys:?}"
        );
    }
    let root = Temp::new()?;
    let mut opts = options(&root, "http://127.0.0.1:1/v1");
    opts.environment = vec![];
    opts.model = "haiku".into();
    let failure = serde_json::to_value(sent(&root, &opts).err().ok_or("sent without a key")?)?;
    assert_eq!(failure["id"], "secrets.keyring_read");
    Ok(())
}

#[test]
fn keyless_resolution_names_every_missing_key_and_redacts_the_used_one() -> Test {
    let root = Temp::new()?;
    let mut opts = options(&root, "http://127.0.0.1:1/v1");
    opts.environment = vec![];
    opts.model = "haiku".into();
    let logger = logger(&root)?;
    let empty = |_: &str| Ok(String::new());
    let failure = {
        let mut run = run(opts.clone(), &logger)?;

        run.keyring = Box::new(empty);
        run.send(&request(), Some(false))
    }
    .err()
    .ok_or("sent without a key")?;
    let failure = serde_json::to_value(failure)?;
    assert_eq!(failure["id"], "secrets.key_missing");
    let message = failure["message"].as_str().ok_or("message")?;
    for variable in [
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "TOGETHER_API_KEY",
        "NOUS_API_KEY",
    ] {
        assert!(message.contains(variable), "{message}");
    }
    let stub = Stub::new(vec![http(
        401,
        json!({"error":"controlled lower refusal with nous-secret; keep this detail"}),
    )])?;
    let mut opts = single_reply_options(&root, &stub.url);
    opts.environment = vec![("NOUS_API_KEY".into(), "nous-secret".into())];
    opts.model = "luna".into();
    let failure = {
        let mut run = run(opts.clone(), &logger)?;

        run.keyring = Box::new(empty);
        run.send(&request(), Some(false))
    }
    .err()
    .ok_or("401 accepted")?;
    let failure = serde_json::to_value(failure)?;
    let stub_outcome = stub.finish();
    let diagnostic = format!("failure={failure}; stub={stub_outcome:?}");
    assert_eq!(failure["id"], "http.auth", "{diagnostic}");
    let message = failure["message"].as_str().ok_or("message")?;
    assert!(
        message.contains("controlled lower refusal with [redacted]; keep this detail"),
        "{diagnostic}"
    );
    assert!(
        message.contains("[redacted]") && !message.contains("nous-secret"),
        "{diagnostic}"
    );
    assert_eq!(stub_outcome?.len(), 1);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn compat_reply_missing_the_schema_is_schema_mismatch() -> Test {
    for (provider, model, raw) in [
        (
            "openai",
            "luna",
            json!({"choices":[{"message":{"content":"{}"}}]}),
        ),
        (
            "anthropic",
            "haiku",
            json!({"content":[{"type":"text","text":"{}"}]}),
        ),
        ("typesafe", "jev", json!({})),
    ] {
        let root = Temp::new()?;
        let stub = Stub::new(vec![http(200, raw)])?;
        let opts = options(&root, &stub.url);
        let failure = with_route(&opts, (provider, model), &stub.url, |route| {
            match route.provider.wire {
                sc_agent::Wire::Jev => Jev(route).decide(&request()),
                sc_agent::Wire::Chat => OpenAiCompat(route).decide(&request()),
                sc_agent::Wire::Messages => AnthropicCompat(route).decide(&request()),
            }
        })?;
        assert_eq!(result(failure)["error"]["id"], "decision.schema_mismatch");
        assert_eq!(stub.finish()?.len(), 1);
    }
    Ok(())
}

#[test]
fn system_prompt_override_by_full_id() -> Test {
    for (provider, model, key, raw, at) in [
        (
            "openai",
            "gpt-6-luna",
            "luna-system-prompt",
            json!({"choices":[{"message":{"content":answer().to_string()}}]}),
            "/messages/0/content",
        ),
        (
            "anthropic",
            "claude-haiku-5-5",
            "haiku-system-prompt",
            json!({"content":[{"type":"text","text":answer().to_string()}]}),
            "/system",
        ),
    ] {
        let root = Temp::new()?;
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
        opts.settings
            .decidr
            .insert(key.into(), "full-id prompt".into());
        assert_eq!(sent(&root, &opts)?["answers"], answer()["answers"]);
        let seen = stub.finish()?;
        assert_eq!(seen[0].body.pointer(at), Some(&json!("full-id prompt")));
    }
    Ok(())
}

#[test]
fn body_read_failure_is_resent() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![Reply::Truncated, Reply::Truncated])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 2;
    let failure = result(sent(&root, &opts));
    assert_eq!(failure["error"]["id"], "http.network");
    assert_eq!(failure["error"]["attempts"], 2);
    let seen = stub.finish()?;
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].body, seen[1].body);
    Ok(())
}

#[test]
fn configurable_send_count_does_not_overflow_the_backoff() -> Test {
    use std::{sync::Mutex, time::Duration};
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(
        (0..66)
            .map(|_| http(502, json!({"error":"busy"})))
            .collect(),
    )?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 66;
    let waits = std::sync::Arc::new(Mutex::new(Vec::new()));
    let failure = result({
        let mut run = run(opts.clone(), &logger)?;

        let waits = waits.clone();
        run.wait = Box::new(move |delay| {
            if let Ok(mut waits) = waits.lock() {
                waits.push(delay);
            }
            Ok(())
        });

        run.send(&request(), Some(false))
    });
    assert_eq!(failure["error"]["id"], "http.provider_busy");
    assert_eq!(failure["error"]["attempts"], 66);
    assert_eq!(stub.finish()?.len(), 66);
    let waits = waits
        .lock()
        .map(|v| v.clone())
        .map_err(|_| "waits recorder poisoned")?;
    assert_eq!(waits.len(), 65);
    assert_eq!(waits.last(), Some(&Duration::from_secs(u64::MAX)));
    assert!(waits.windows(2).all(|pair| pair[0] < pair[1]));
    logger.shutdown()?;
    Ok(())
}

#[test]
fn redirect_never_forwards_the_api_key_to_another_endpoint() -> Test {
    let root = Temp::new()?;
    let target = Stub::new(vec![http(
        200,
        json!({"content":[{"type":"text","text":"{}"}]}),
    )])?;
    let source = Stub::new(vec![Reply::Redirect(target.url.clone())])?;
    let mut opts = options(&root, &source.url);
    opts.settings.max_sends = 1;
    let outcome = with_route(&opts, ("anthropic", "haiku"), &source.url, |route| {
        AnthropicCompat(route).call(&llm_request())
    })?;
    let original = source.finish()?;
    let redirected = target.finish()?;
    assert_eq!(original.len(), 1);
    assert_eq!(header(&original[0], "x-api-key"), Some("test-secret"));
    assert!(redirected.is_empty(), "redirect target received a request");
    assert_eq!(result(outcome)["error"]["id"], "decision.api_status");
    Ok(())
}

#[test]
fn anthropic_compat_skips_thinking_blocks_before_the_first_text_answer() -> Test {
    for model in ["haiku", "sonnet", "opus", "fable"] {
        let root = Temp::new()?;
        let raw = json!({"content":[
            {"type":"thinking","thinking":"internal reasoning"},
            {"type":"text","text":42},
            {"type":"text","text":answer().to_string()},
            {"type":"text","text":"not the first text"}
        ]});
        let stub = Stub::new(vec![http(200, raw.clone()), http(200, raw)])?;
        let opts = options(&root, &stub.url);
        with_route(&opts, ("anthropic", model), &stub.url, |route| -> Test {
            assert_eq!(AnthropicCompat(route).call(&llm_request())?, answer());
            assert_eq!(AnthropicCompat(route).decide(&request())?, answer());
            Ok(())
        })??;
        assert_eq!(stub.finish()?.len(), 2);
    }
    Ok(())
}

#[test]
fn llm_decision_backends_reject_answers_outside_the_requested_schema() -> Test {
    for (question, answer) in [
        (
            json!({"type":"choice","criteria":{"a":"A"}}),
            json!({"type":"choice","choice":"outside","confidence":0.8,"probabilities":{}}),
        ),
        (json!({"type":"box"}), json!({"type":"box","box":null})),
    ] {
        for (provider, model) in [("openai", "luna"), ("anthropic", "haiku")] {
            let root = Temp::new()?;
            let reply = json!({"answers":{"q":answer}}).to_string();
            let wire = pinned_model(provider, model)?.0.wire;
            let raw = match wire {
                sc_agent::Wire::Messages => json!({"content":[{"type":"text","text":reply}]}),
                sc_agent::Wire::Chat => json!({"choices":[{"message":{"content":reply}}]}),
                sc_agent::Wire::Jev => return Err("expected an LLM wire".into()),
            };
            let stub = Stub::new(vec![http(200, raw)])?;
            let opts = options(&root, &stub.url);
            let request = json!({"questions":{"q":question}});
            let failure = with_route(&opts, (provider, model), &stub.url, |route| match wire {
                sc_agent::Wire::Messages => AnthropicCompat(route).decide(&request),
                sc_agent::Wire::Chat => OpenAiCompat(route).decide(&request),
                sc_agent::Wire::Jev => Jev(route).decide(&request),
            })?;
            assert_eq!(result(failure)["error"]["id"], "decision.schema_mismatch");
            assert_eq!(stub.finish()?.len(), 1);
        }
    }
    Ok(())
}

#[test]
fn stub_shutdown_wakes_a_worker_with_unserved_replies() -> Test {
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    assert!(stub.finish()?.is_empty());
    Ok(())
}
