//! Quota detection for busy replies (REQ-SAG-0602).
//!
//! A busy status (429, 502, 529) is resent, unless its body says the account
//! is out of tokens, credit or spend allowance: no wait makes that succeed, so
//! [`is_quota_reply`] turns it into `http.quota_exhausted` at once. It is
//! consulted only for busy statuses; every other status is a final reply
//! that the backend decodes. The same [`Body`] reader gives
//! [`ProviderReply`](crate::ProviderReply) its fields.
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
//! - **Jev** (`plan-anchor`'s `jev_judge.py.j2`, `http_error`): 401 is a
//!   credential failure; 422 carries the field error text; 429 and 529 are
//!   busy; 5xx is transient (the backend decodes every non-busy reply). Jev's
//!   own error JSON is not documented here, so the reader also accepts
//!   `{"detail": "..."}`, `{"detail": [{"loc": [...], "msg": "..."}]}` and
//!   `{"message": "..."}`, and falls back to the raw body. Live replies on 2026-11-08
//!   showed `{"detail": {"error_type": "billing_error", "message": "..."}}`
//!   on 402 and the same shape with `authentication_error` on 401.
//!
//! A body that is none of these stays what its status says: an unknown 429
//! body is busy, never quota.

use serde_json::Value;

/// `error.code`, `error.type` or `error.details.error_code` values that
/// mean the account is out of tokens, credit or spend allowance.
const QUOTA_MARKERS: [&str; 4] = [
    "insufficient_quota",
    "credit_balance_exhausted",
    "billing_error",
    "enforced_spend_limit_reached",
];

/// Message starts (lower case) of a busy reply that mean out of quota:
/// Anthropic's credit-balance text, reported by users but not listed in its
/// docs. Anthropic's customer spend limit (a 400 whose message starts "You
/// have reached your specified") is not here: a 400 is a final reply.
const QUOTA_MESSAGES: [&str; 1] = ["your credit balance is too low"];

/// What the reader found in a failing reply's body.
#[derive(Debug, Default)]
pub(crate) struct Body {
    pub(crate) provider_type: Option<String>,
    pub(crate) provider_code: Option<String>,
    detail_code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) param: Option<String>,
    pub(crate) request_id: Option<String>,
}

fn string_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

impl Body {
    /// The fields of a provider error body; all `None` when it is not JSON.
    pub(crate) fn read(bytes: &[u8]) -> Self {
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

    fn marks_quota(&self) -> bool {
        let marker = |value: &Option<String>| {
            value
                .as_deref()
                .is_some_and(|text| QUOTA_MARKERS.contains(&text))
        };
        if marker(&self.provider_code) || marker(&self.provider_type) || marker(&self.detail_code) {
            return true;
        }
        self.message.as_deref().is_some_and(|message| {
            let lower = message.to_ascii_lowercase();
            QUOTA_MESSAGES.iter().any(|start| lower.starts_with(start))
        })
    }
}

/// Whether a busy reply with `status` and `body` says the account is out of
/// tokens, credit or spend allowance.
///
/// Only a busy status (429, 502, 529) can be a quota reply: a 402 or
/// Anthropic's usage-limit 400 is a final reply, returned to the backend
/// after one send (REQ-SAG-0601). A busy reply for which this returns `true`
/// is `http.quota_exhausted` instead of being resent (REQ-SAG-0602).
pub(crate) fn is_quota_reply(status: u16, body: &[u8]) -> bool {
    crate::resend::is_busy_status(status) && Body::read(body).marks_quota()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_markers_come_from_documented_shapes_only() {
        let anthropic_cap = r#"{"type":"error","error":{"type":"rate_limit_error","message":"You have reached your API usage limits","details":{"error_code":"enforced_spend_limit_reached"}}}"#;
        let openai_quota = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","param":null,"code":"insufficient_quota"}}"#;
        let openai_credit = r#"{"error":{"message":"no credit","type":"billing","code":"credit_balance_exhausted"}}"#;
        let anthropic_limit = r#"{"type":"error","error":{"type":"invalid_request_error","message":"You have reached your specified API usage limits. You will regain access on 2026-12-01."}}"#;
        let anthropic_credit = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the API."}}"#;
        for (status, body) in [
            (429, anthropic_cap),
            (429, openai_quota),
            (429, openai_credit),
            (529, anthropic_credit),
            (502, openai_quota),
        ] {
            assert!(is_quota_reply(status, body.as_bytes()), "{status} {body}");
        }
        // Final replies are never quota here: the backend reads them.
        for (status, body) in [
            (400, anthropic_limit),
            (402, "{}"),
            (402, openai_quota),
            (400, anthropic_credit),
        ] {
            assert!(!is_quota_reply(status, body.as_bytes()), "{status} {body}");
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
}
