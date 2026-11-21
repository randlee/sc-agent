//! A normal local tool loop, written once against `Llm`, run over both wires.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Client, Policy};
use sc_agent_types::Error;
use sc_llm_provider::{Chat, ChatCompletions, Llm, Message, Messages, Part, Role, Stop, Tool};
use serde_json::{Value, json};

/// Ask, run each requested tool, send the results back, until the model stops asking.
fn tool_loop<L: Llm>(
    client: &Client<L>,
    question: &str,
    run_tool: impl Fn(&str, &Value) -> String,
) -> Result<String, Error> {
    let mut chat = Chat {
        model: "m".into(),
        max_tokens: 100,
        messages: vec![Message::user(question)],
        tools: vec![Tool {
            name: "add".into(),
            description: "adds".into(),
            input_schema: json!({ "type": "object" }),
        }],
        ..Chat::default()
    };
    loop {
        let reply = client.send(&chat, &Cancel::default())?;
        chat.messages.push(reply.message());
        if reply.stop != Stop::ToolUse {
            return Ok(reply.text());
        }
        let results = reply
            .content
            .iter()
            .filter_map(|p| match p {
                Part::ToolUse { id, name, input } => Some(Part::ToolResult {
                    id: id.clone(),
                    content: run_tool(name, input),
                    is_error: false,
                }),
                _ => None,
            })
            .collect();
        chat.messages.push(Message {
            role: Role::User,
            content: results,
        });
    }
}

fn add(_: &str, input: &Value) -> String {
    (input["a"].as_i64().unwrap() + input["b"].as_i64().unwrap()).to_string()
}

fn policy() -> Policy {
    Policy {
        max_sends: 1,
        backoff: Duration::ZERO,
        timeout: Duration::from_secs(5),
    }
}

#[test]
fn haiku_wire_runs_the_tool_loop() {
    let server = Server::start(vec![
        ok(
            json!({ "content": [{ "type": "text", "text": "let me add" }, { "type": "tool_use", "id": "t1", "name": "add", "input": { "a": 2, "b": 3 } }],
            "stop_reason": "tool_use", "usage": { "input_tokens": 5, "output_tokens": 6 } }),
        ),
        ok(
            json!({ "content": [{ "type": "text", "text": "5" }], "stop_reason": "end_turn", "usage": { "input_tokens": 1, "output_tokens": 1 } }),
        ),
    ]);
    let client = Client::new(
        Messages::new(server.base.clone(), Some("k".into())),
        policy(),
    );
    assert_eq!(tool_loop(&client, "2+3?", add).unwrap(), "5");
    let seen = server.finish();
    assert_eq!(seen[0].path, "/messages");
    assert_eq!(seen[0].header("x-api-key"), Some("k"));
    assert_eq!(seen[0].json()["tools"][0]["name"], "add");
    let second = seen[1].json();
    assert_eq!(
        second["messages"][1]["content"][1],
        json!({ "type": "tool_use", "id": "t1", "name": "add", "input": { "a": 2, "b": 3 } })
    );
    assert_eq!(
        second["messages"][2]["content"][0],
        json!({ "type": "tool_result", "tool_use_id": "t1", "content": "5", "is_error": false })
    );
}

#[test]
fn luna_wire_runs_the_same_tool_loop() {
    let server = Server::start(vec![
        ok(
            json!({ "choices": [{ "message": { "content": null, "tool_calls": [{ "id": "c1", "type": "function",
            "function": { "name": "add", "arguments": "{\"a\":2,\"b\":3}" } }] }, "finish_reason": "tool_calls" }],
            "usage": { "prompt_tokens": 5, "completion_tokens": 6 } }),
        ),
        ok(json!({ "choices": [{ "message": { "content": "5" }, "finish_reason": "stop" }] })),
    ]);
    let client = Client::new(
        ChatCompletions::new(server.base.clone(), Some("k".into()), true),
        policy(),
    );
    assert_eq!(tool_loop(&client, "2+3?", add).unwrap(), "5");
    let seen = server.finish();
    assert_eq!(seen[0].path, "/chat/completions");
    assert_eq!(seen[0].header("authorization"), Some("Bearer k"));
    assert_eq!(seen[0].json()["max_completion_tokens"], 100);
    assert_eq!(seen[0].json()["tools"][0]["function"]["name"], "add");
    let second = seen[1].json();
    assert_eq!(
        second["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{\"a\":2,\"b\":3}"
    );
    assert_eq!(
        second["messages"][2],
        json!({ "role": "tool", "tool_call_id": "c1", "content": "5" })
    );
}

#[test]
fn backend_is_chosen_by_wire_and_checked_once() {
    let anthropic = sc_agent_types::providers::Provider::by_name("anthropic").unwrap();
    assert!(sc_llm_provider::backend(anthropic, Some("k".into())).is_ok());
    let typesafe = sc_agent_types::providers::Provider::by_name("typesafe").unwrap();
    assert!(matches!(
        sc_llm_provider::backend(typesafe, None),
        Err(Error::Config(_))
    ));
}

#[test]
fn image_and_schema_are_encoded_per_wire() {
    let chat = Chat {
        model: "m".into(),
        max_tokens: 9,
        system: Some("s".into()),
        messages: vec![Message {
            role: Role::User,
            content: vec![Part::Image {
                media_type: "image/png".into(),
                data: "QUJD".into(),
            }],
        }],
        output_schema: Some(json!({ "type": "object" })),
        ..Chat::default()
    };
    use sc_agent_http::Codec;
    let m = Messages::new("http://127.0.0.1:1".into(), None)
        .encode(&chat)
        .unwrap();
    let m: Value = serde_json::from_slice(m.body()).unwrap();
    assert_eq!(
        m["messages"][0]["content"][0]["source"],
        json!({ "type": "base64", "media_type": "image/png", "data": "QUJD" })
    );
    assert_eq!(m["output_config"]["format"]["type"], "json_schema");
    let c = ChatCompletions::new("http://127.0.0.1:1".into(), None, false)
        .encode(&chat)
        .unwrap();
    let c: Value = serde_json::from_slice(c.body()).unwrap();
    assert_eq!(
        c["messages"][0],
        json!({ "role": "system", "content": "s" })
    );
    assert_eq!(
        c["messages"][1]["content"][0]["image_url"]["url"],
        "data:image/png;base64,QUJD"
    );
    assert_eq!(c["max_tokens"], 9);
    assert_eq!(c["response_format"]["json_schema"]["strict"], true);
}
