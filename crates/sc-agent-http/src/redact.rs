//! Secret redaction of errors.
//!
//! [`HttpClient::send_once`](crate::HttpClient::send_once) passes every error
//! it returns through [`redact`] once, whichever transport produced it.

use std::borrow::Cow;

use sc_agent_secrets::ApiKey;

use crate::error::HttpError;
use crate::request::HttpRequest;

/// Removes the request's key from every text field of `error`.
///
/// Every occurrence of the key in a header echo, a body excerpt or any other
/// text becomes `[redacted]` ([`ApiKey::redact_text`](sc_agent_secrets::ApiKey::redact_text)
/// through [`HttpError::map_text`], so a new variant with a text field is
/// covered there). A request without a secret returns `error` unchanged.
pub(crate) fn redact(error: HttpError, request: &HttpRequest) -> HttpError {
    match request.secret() {
        Some(secret) => {
            let key = secret.key();
            error.map_text(&|text: String| redact_owned(key, text))
        }
        None => error,
    }
}

/// `text` with `key` replaced by `[redacted]`; `text` itself is returned,
/// not copied, when it does not contain the key.
pub(crate) fn redact_owned(key: &ApiKey, text: String) -> String {
    match key.redact_text(&text) {
        Cow::Owned(redacted) => redacted,
        Cow::Borrowed(_) => text,
    }
}

#[cfg(test)]
mod tests {
    use sc_agent_types::ErrorId;

    use super::*;
    use crate::error::NetworkKind;
    use crate::request::{Endpoint, Method, SecretHeader};

    #[test]
    fn key_is_removed_from_every_text() {
        let request = HttpRequest::new(
            Method::Post,
            Endpoint::parse("https://api.openai.com/v1/responses").expect("https"),
            b"{}".as_slice(),
        )
        .with_secret(SecretHeader::bearer(crate::test_key::key("sk-test-1")))
        .expect("https");
        let error =
            HttpError::network(NetworkKind::Connect, 1, "echo: Bearer sk-test-1").with_cause(
                sc_agent_types::ErrorDetail::new("io.Other", "proxy echo sk-test-1", None),
            );
        let redacted = redact(error, &request);
        assert_eq!(
            redacted.message(),
            "connect failure after 1 send: echo: Bearer [redacted]"
        );
        assert_eq!(
            redacted.cause_detail().map(|cause| cause.message),
            Some("proxy echo [redacted]".to_owned())
        );
    }
}
