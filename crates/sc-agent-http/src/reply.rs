//! [`ProviderReply`]: what a provider said in a reply, redacted and kept whole.

use std::fmt;

use sc_agent_secrets::ApiKey;

use crate::quota::Body;
use crate::request::HttpResponse;
use crate::retry_after::RetryAfter;

/// What a provider said in a reply. Text fields are redacted and otherwise
/// kept whole (REQ-SAG-0002).
///
/// Built by [`ProviderReply::read`] from the status, the headers and the
/// body. When the body is not a provider's JSON error shape, `message` holds
/// the raw body text (invalid UTF-8 replaced).
///
/// # Examples
///
/// ```
/// use sc_agent_http::{HttpResponse, ProviderReply};
///
/// let response = HttpResponse::new(
///     429,
///     vec![("Retry-After".to_owned(), "2.5".to_owned())],
///     br#"{"error":{"type":"requests","code":"rate_limit_exceeded","message":"slow down"}}"#.to_vec(),
/// );
/// let reply = ProviderReply::read(&response, None);
/// assert_eq!(reply.status, 429);
/// assert_eq!(reply.provider_code.as_deref(), Some("rate_limit_exceeded"));
/// assert_eq!(reply.message, "slow down");
/// assert_eq!(reply.retry_after_seconds, Some(2.5));
/// assert_eq!(reply.to_string(), "HTTP 429 (requests, rate_limit_exceeded): slow down");
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ProviderReply {
    /// The HTTP status.
    pub status: u16,
    /// The provider's error type (`invalid_request_error`), when it sent one.
    pub provider_type: Option<String>,
    /// The provider's error code (`insufficient_quota`), when it sent one.
    pub provider_code: Option<String>,
    /// The provider's message, or the raw body.
    pub message: String,
    /// The request field the provider blamed, when it named one.
    pub param: Option<String>,
    /// The provider's request id (body `request_id`, else the `request-id`
    /// or `x-request-id` header), when it sent one.
    pub request_id: Option<String>,
    /// A valid `Retry-After` as sent, in seconds, unrounded (REQ-SAG-0007).
    pub retry_after_seconds: Option<f64>,
}

impl ProviderReply {
    /// Reads the status, type, code, message, parameter, request id and
    /// `Retry-After` of `response`, with every text field redacted of
    /// `secret`.
    ///
    /// Public because sc-llm-provider reads non-2xx provider replies with
    /// it. Nothing is cut: the key is replaced in each whole text, so no
    /// fragment of it can survive (REQ-SAG-0002).
    #[must_use]
    pub fn read(response: &HttpResponse, secret: Option<&ApiKey>) -> Self {
        let scrub = |text: &str| match secret {
            Some(key) => key.redact_text(text).into_owned(),
            None => text.to_owned(),
        };
        let bytes = response.body();
        let body = Body::read(bytes);
        let message = match body.message.as_deref() {
            Some(text) => scrub(text),
            None => scrub(&String::from_utf8_lossy(bytes)),
        };
        let request_id = body
            .request_id
            .as_deref()
            .or_else(|| response.header("request-id"))
            .or_else(|| response.header("x-request-id"))
            .map(scrub);
        Self {
            status: response.status(),
            provider_type: body.provider_type.as_deref().map(scrub),
            provider_code: body.provider_code.as_deref().map(scrub),
            message,
            param: body.param.as_deref().map(scrub),
            request_id,
            retry_after_seconds: RetryAfter::parse(response.header("retry-after"))
                .map(RetryAfter::seconds),
        }
    }

    /// This reply with `map` applied to every text field (redaction).
    pub(crate) fn map_text(self, map: &impl Fn(&str) -> String) -> Self {
        Self {
            status: self.status,
            provider_type: self.provider_type.as_deref().map(map),
            provider_code: self.provider_code.as_deref().map(map),
            message: map(&self.message),
            param: self.param.as_deref().map(map),
            request_id: self.request_id.as_deref().map(map),
            retry_after_seconds: self.retry_after_seconds,
        }
    }
}

impl fmt::Display for ProviderReply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "HTTP {}", self.status)?;
        match (&self.provider_type, &self.provider_code) {
            (Some(kind), Some(code)) => write!(formatter, " ({kind}, {code})")?,
            (Some(text), None) | (None, Some(text)) => write!(formatter, " ({text})")?,
            (None, None) => {}
        }
        if !self.message.is_empty() {
            write!(formatter, ": {}", self.message)?;
        }
        if let Some(id) = &self.request_id {
            write!(formatter, " (request id {id})")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-big-body-secret-0123456789";

    fn key() -> ApiKey {
        crate::test_key::key(KEY)
    }

    /// A body far over any old read cut, with the key at many offsets:
    /// nothing of the key survives, and nothing of the text is cut.
    #[test]
    fn key_in_a_large_raw_body_is_scrubbed_and_kept_whole() {
        let key = key();
        for lead in [1000_usize, 1010, 1020, 4090, 4096, 4100] {
            let body = format!("{}{KEY}{}", "a".repeat(lead), "z".repeat(6000));
            let reply =
                ProviderReply::read(&HttpResponse::new(500, Vec::new(), body.into()), Some(&key));
            let rendered = format!("{reply} {reply:?}");
            for size in 4..=KEY.len() {
                assert!(
                    !rendered.contains(&KEY[..size])
                        && !rendered.contains(&KEY[KEY.len() - size..]),
                    "lead {lead}: a {size}-byte key fragment survived"
                );
            }
            assert_eq!(reply.message.len(), lead + "[redacted]".len() + 6000);
        }
    }

    #[test]
    fn every_text_field_is_scrubbed() {
        let body = format!(
            r#"{{"error":{{"type":"t {KEY}","code":"c {KEY}","message":"m {KEY}","param":"p {KEY}"}},"request_id":"r {KEY}"}}"#
        );
        let reply = ProviderReply::read(
            &HttpResponse::new(400, Vec::new(), body.into()),
            Some(&key()),
        );
        assert!(!format!("{reply:?}").contains(KEY), "{reply:?}");
        assert_eq!(reply.message, "m [redacted]");
        assert_eq!(reply.request_id.as_deref(), Some("r [redacted]"));
    }

    #[test]
    fn request_id_falls_back_to_headers() {
        let response = HttpResponse::new(
            503,
            vec![("X-Request-Id".to_owned(), "abc".to_owned())],
            b"down".to_vec(),
        );
        let reply = ProviderReply::read(&response, None);
        assert_eq!(reply.request_id.as_deref(), Some("abc"));
        assert_eq!(reply.message, "down");
        assert_eq!(reply.retry_after_seconds, None);
    }
}
