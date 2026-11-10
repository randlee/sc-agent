fn response(status: u16, headers: Vec<(&str, &str)>, body: Value) -> HttpResponse {
    text_response(
        status,
        headers,
        &serde_json::to_string(&body).expect("json"),
    )
}

fn text_response(status: u16, headers: Vec<(&str, &str)>, body: &str) -> HttpResponse {
    HttpResponse::new(
        status,
        headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body.as_bytes().to_vec(),
    )
}
fn haiku_message(text: &str, stop_reason: &str, input_tokens: i64) -> HttpResponse {
    response(
        200,
        Vec::new(),
        json!({
            "stop_reason": stop_reason,
            "content": [{ "type": "text", "text": text }],
            "usage": {
                "input_tokens": input_tokens,
                "cache_read_input_tokens": 2,
                "output_tokens": 9
            }
        }),
    )
}
#[test]
fn test_llm_stop_reason_truncation_is_response_invalid() {
    let (haiku_backend, haiku_recorded) = haiku(vec![
        haiku_message(&answer_text(), "max_tokens", 8),
        count_reply(8),
    ]);
    let haiku_error = haiku_backend
        .judge(&standard_request(PROMPT), &SendTrace::new())
        .expect_err("truncated haiku");
    assert_eq!(haiku_error.code(), "RESPONSE_INVALID");
    assert_eq!(
        haiku_error.details().and_then(|d| d.cause).as_deref(),
        Some("truncated")
    );
    assert!(!haiku_error.retryable());
    assert!(haiku_error.to_string().contains("max_tokens"));
    assert!(haiku_error.to_string().contains("stop_reason"));
    assert_eq!(posts(&haiku_recorded).len(), 2);

    let (luna_backend, luna_recorded) = luna(vec![
        luna_message(&answer_text(), "incomplete", 8),
        count_reply(8),
    ]);
    let luna_error = luna_backend
        .judge(&standard_request(PROMPT), &SendTrace::new())
        .expect_err("truncated luna");
    assert_eq!(luna_error.code(), "RESPONSE_INVALID");
    assert_eq!(
        luna_error.details().and_then(|d| d.cause).as_deref(),
        Some("truncated")
    );
    assert!(luna_error.to_string().contains("max_tokens"));
    assert!(luna_error.to_string().contains("incomplete"));
    assert_eq!(posts(&luna_recorded).len(), 2);
}

