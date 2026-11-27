pub mod support;
use sc_agent::{
    AnthropicCompat, Cancel, DEFAULT_SYSTEM_PROMPT, Decision, Jev, Llm, LlmRequest, OpenAiCompat,
    Options, Route, model_with, result, send,
};
use serde_json::{Value, json};
use std::cell::Cell;
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
    let (provider, model) = model_with(name, Some(provider), &Default::default())?;
    let sends = Cell::new(0);
    let route = Route {
        options,
        provider,
        model,
        base: base.into(),
        cancel: &Cancel::default(),
        wait: &no_wait,
        keyring: &keyring,
        sends: &sends,
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
fn openai_compat_call_returns_the_json_the_model_wrote() -> Test {
    let root = Temp::new()?;
    let raw = json!({"choices":[{"message":{"content":"{\"a\":1}"}}]});
    let stub = Stub::new(vec![http(200, raw)])?;
    let opts = options(&root, &stub.url);
    let reply = with_route(&opts, ("openai", "luna"), &stub.url, |route| {
        OpenAiCompat(route).call(&llm_request())
    })??;
    assert_eq!(reply, json!({"a":1}));
    let seen = stub.finish()?;
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
            let value = send(
                &request(),
                &opts,
                &logger,
                &Cancel::default(),
                &no_wait,
                &keyring,
            )?;
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
    let failure = result(send(
        &request(),
        &opts,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    ));
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
    let failure = result(send(
        &request(),
        &opts,
        &logger,
        &Cancel::default(),
        &no_wait,
        &keyring,
    ));
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
