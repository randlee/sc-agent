//! Plain Chat/Messages contracts, without tool-loop scaffolding.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Client, Codec, Policy};
use sc_agent_types::Error;
use sc_llm_provider::{Chat, ChatCompletions, Message, Messages, Part, Role};
use serde_json::{Value, json};

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
    };
    let m = Messages::new("http://127.0.0.1:1".into(), None)
        .encode(&chat)
        .unwrap();
    let m: Value = serde_json::from_slice(m.body()).unwrap();
    assert_eq!(
        m["messages"][0]["content"][0]["source"],
        json!({ "type": "base64", "media_type": "image/png", "data": "QUJD" })
    );
    assert_eq!(m["output_config"]["format"]["type"], "json_schema");
    for openai in [false, true] {
        let c = ChatCompletions::new("http://127.0.0.1:1".into(), None, openai)
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
        assert_eq!(
            c[if openai {
                "max_completion_tokens"
            } else {
                "max_tokens"
            }],
            9
        );
        assert_eq!(c["response_format"]["json_schema"]["strict"], true);
    }
}

#[test]
fn messages_text_wire() {
    let server = Server::start(vec![ok(
        json!({"content":[{"type":"text","text":"hello"}]}),
    )]);
    let client = Client::new(
        Messages::new(server.base.clone(), Some("k".into())),
        Policy::default(),
    );
    let reply = client.send(&Chat::default(), &Cancel::default()).unwrap();
    assert_eq!(reply.text(), "hello");
    let seen = server.finish();
    assert_eq!(seen[0].path, "/messages");
    assert_eq!(seen[0].header("x-api-key"), Some("k"));
}

#[test]
fn chat_text_wire() {
    let server = Server::start(vec![ok(
        json!({"choices":[{"message":{"content":"hello"}}]}),
    )]);
    let client = Client::new(
        ChatCompletions::new(server.base.clone(), Some("k".into()), true),
        Policy::default(),
    );
    let reply = client.send(&Chat::default(), &Cancel::default()).unwrap();
    assert_eq!(reply.text(), "hello");
    let seen = server.finish();
    assert_eq!(seen[0].path, "/chat/completions");
    assert_eq!(seen[0].header("authorization"), Some("Bearer k"));
}
