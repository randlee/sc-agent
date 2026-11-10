//! Classification of a provider reply with a failing status.
//!
//! One place decides which error id a non-200 reply from Jev, Haiku or Luna
//! becomes, so no backend maps statuses on its own. The decision uses the
//! status first and the body second, and keeps what the provider said.
//!
//! # Mapping
//!
//! | Reply | Error |
//! | --- | --- |
//! | 401, 403 | `AUTH_FAILED` |
//! | 402, or a quota body (below) | `QUOTA_EXHAUSTED` |
//! | 413, or 400/422 with a structured marker (`context_length_exceeded`, `request_too_large`), or, for Haiku and Luna ([`ReplyShape::Llm`]), Anthropic's `invalid_request_error` whose message starts "prompt is too long" (unverified wording), or, for Jev only ([`ReplyShape::Jev`]), a message naming `max_tokens` or tokens exceeded (below) | `INPUT_TOO_LARGE` (unit `bytes` for 413, else `tokens`) |
//! | 400, 422 | `API_REQUEST_INVALID` |
//! | every other status, 5xx included | `API_ERROR` |
//!
//! 429, 502 and 529 are busy and retried by [`send_with_busy_retry`]
//! ([`crate::send_with_busy_retry`]); they reach this module only with a
//! quota body, which [`retry`](crate::retry) hands over instead of retrying.
//!
//! # Documented body shapes
//!
//! The bodies below are the shapes the providers document, not observed
//! against a live endpoint here, except Jev's `detail` object, below.
//!
//! - **Anthropic** ([errors](https://platform.claude.com/docs/en/api/errors),
//!   [rate limits](https://platform.claude.com/docs/en/api/rate-limits)):
//!   `{"type":"error","error":{"type":"...","message":"...","details":{"error_code":"..."}},"request_id":"req_..."}`.
//!   The tier spend cap is a 429 `rate_limit_error` with
//!   `error.details.error_code == "enforced_spend_limit_reached"` and no
//!   `retry-after` header; a spend limit the customer set is a 400
//!   `invalid_request_error` whose message starts `You have reached your
//!   specified`; 402 is `billing_error`; 413 is `request_too_large`.
//! - **OpenAI** ([error codes](https://developers.openai.com/api/docs/guides/error-codes)):
//!   `{"error":{"message":"...","type":"...","param":null,"code":"..."}}`.
//!   A 429 with `code` `insufficient_quota` or `credit_balance_exhausted` is
//!   out of quota; a 429 with `rate_limit_exceeded` is a rate limit. A 400
//!   `context_length_exceeded` is a too-long input (that code is widely
//!   reported but not listed on the page above).
//! - **Jev** (`plan-anchor`'s `jev_judge.py.j2`, `http_error`): a 4xx whose
//!   message mentions `max_tokens`, or `token` and `exceed`, is a context
//!   overflow (matched on the message text, never on JSON keys, and only for
//!   [`ReplyShape::Jev`]; for Haiku and Luna a message about `max_tokens`, the
//!   output cap, is an ordinary invalid request); 401 is a credential failure; 422 carries the field error text;
//!   429 and 529 are busy; 5xx is transient. Jev's own error JSON is not
//!   documented here, so the reader also accepts `{"detail": "..."}`,
//!   `{"detail": [{"loc": [...], "msg": "..."}]}` and `{"message": "..."}`,
//!   and falls back to the bounded raw body. Live replies on 2026-11-08
//!   showed `{"detail": {"error_type": "billing_error", "message": "..."}}`
//!   on 402 and the same shape with `authentication_error` on 401.
//!
//! A body that is none of these stays what its status says: an unknown 429
//! body is busy, never quota.
/// `error.code`, `error.type` or `error.details.error_code` values that
/// mean the account is out of tokens, credit or spend allowance.
const QUOTA_MARKERS: [&str; 4] = [
    "insufficient_quota",
    "credit_balance_exhausted",
    "billing_error",
    "enforced_spend_limit_reached",
];

/// Message starts (lower case) that mean out of quota. Anthropic's customer
/// spend limit is a 400 with the first; the second is Anthropic's
/// credit-balance text, reported by users but not listed in its docs.
const QUOTA_MESSAGES: [&str; 2] = [
    "you have reached your specified",
    "your credit balance is too low",
];

/// What the reader found in a failing reply's body.
#[derive(Debug, Default)]
struct Body {
    provider_type: Option<String>,
    provider_code: Option<String>,
    detail_code: Option<String>,
    message: Option<String>,
    param: Option<String>,
    request_id: Option<String>,
}

fn string_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

impl Body {
    fn read(bytes: &[u8]) -> Self {
        let Ok(Value::Object(root)) = serde_json::from_slice::<Value>(bytes) else {
            return Self::default();
        };
        let mut body = Self {
            request_id: root.get("request_id").and_then(string_of),
            ..Self::default()
        };
        match root.get("error") {
            Some(Value::Object(error)) => {
                body.provider_type = error.get("type").and_then(string_of);
                body.provider_code = error.get("code").and_then(string_of);
                body.message = error.get("message").and_then(string_of);
                body.param = error.get("param").and_then(string_of);
                body.detail_code = error
                    .get("details")
                    .and_then(|details| details.get("error_code"))
                    .and_then(string_of);
            }
            Some(Value::String(text)) if !text.is_empty() => body.message = Some(text.clone()),
            _ => {}
        }
        if body.message.is_none() {
            body.message = root.get("message").and_then(string_of);
        }
        if body.provider_code.is_none() {
            body.provider_code = root.get("code").and_then(string_of);
        }
        if body.message.is_none() {
            match root.get("detail") {
                Some(Value::String(text)) if !text.is_empty() => body.message = Some(text.clone()),
                // Jev's billing and auth errors, observed live 2026-11-08:
                // {"detail":{"error_type":"billing_error","message":"..."}}.
                Some(Value::Object(detail)) => {
                    if body.provider_type.is_none() {
                        body.provider_type = detail.get("error_type").and_then(string_of);
                    }
                    body.message = detail.get("message").and_then(string_of);
                }
                Some(Value::Array(items)) => {
                    let first = items.first();
                    body.message = first.and_then(|item| item.get("msg")).and_then(string_of);
                    body.param = first
                        .and_then(|item| item.get("loc"))
                        .and_then(Value::as_array)
                        .and_then(|loc| loc.last())
                        .and_then(string_of);
                }
                _ => {}
            }
        }
        body
    }

    fn marks_quota(&self, status: u16) -> bool {
        let marker = |value: &Option<String>| {
            value
                .as_deref()
                .is_some_and(|text| QUOTA_MARKERS.contains(&text))
        };
        if marker(&self.provider_code) || marker(&self.provider_type) || marker(&self.detail_code) {
            return true;
        }
        matches!(status, 400 | 429)
            && self.message.as_deref().is_some_and(|message| {
                let lower = message.to_ascii_lowercase();
                QUOTA_MESSAGES.iter().any(|start| lower.starts_with(start))
            })
    }
}

/// Whether a reply with `status` and `body` says the account is out of
/// tokens, credit or spend allowance.
///
/// This is the classification hook of the busy retry: a 429 for which this
/// returns `true` is handed to the caller instead of being retried, because
/// no wait will make it succeed.
pub(crate) fn is_quota_reply(status: u16, body: &[u8]) -> bool {
    status == 402 || Body::read(body).marks_quota(status)
}
    #[test]
    fn quota_markers_come_from_documented_shapes_only() {
        let anthropic_cap = r#"{"type":"error","error":{"type":"rate_limit_error","message":"You have reached your API usage limits","details":{"error_code":"enforced_spend_limit_reached"}}}"#;
        let openai_quota = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","param":null,"code":"insufficient_quota"}}"#;
        let openai_credit = r#"{"error":{"message":"no credit","type":"billing","code":"credit_balance_exhausted"}}"#;
        let anthropic_limit = r#"{"type":"error","error":{"type":"invalid_request_error","message":"You have reached your specified API usage limits. You will regain access on 2026-12-01."}}"#;
        for (status, body) in [
            (429, anthropic_cap),
            (429, openai_quota),
            (429, openai_credit),
            (400, anthropic_limit),
            (402, "{}"),
        ] {
            assert!(is_quota_reply(status, body.as_bytes()), "{status} {body}");
            assert_eq!(
                api_failure("p", ReplyShape::Llm, &req(), &reply(status, body)).code(),
                "QUOTA_EXHAUSTED"
            );
        }
        let rate = r#"{"error":{"message":"Rate limit reached","type":"requests","code":"rate_limit_exceeded"}}"#;
        let anthropic_rate =
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
        for body in [
            rate,
            anthropic_rate,
            "",
            "not json",
            "{}",
            r#"{"error":"x"}"#,
        ] {
            assert!(!is_quota_reply(429, body.as_bytes()), "{body}");
        }
    }
