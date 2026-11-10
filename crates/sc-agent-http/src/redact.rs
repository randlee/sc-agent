//! Secret redaction of errors.
//!
//! [`send_with_busy_retry`](crate::send_with_busy_retry) passes every error it
//! returns through [`redact`] once, whichever transport produced it.

use sc_agent_types::JevError;

use crate::request::HttpRequest;

/// Removes the request's secret from every text field of `error`.
///
/// Every occurrence of the key in a header echo, a body excerpt or any other
/// detail text becomes `[redacted]`. Built on
/// [`ResolvedApiKey::redact_error`](sc_agent_types::ResolvedApiKey::redact_error),
/// so a new error variant with a text field is covered without a change
/// here. A request without a secret returns `error` unchanged.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use sc_agent_http::{Endpoint, HttpRequest, Method, SecretHeader, redact};
/// use sc_agent_types::{BackendKeyId, JevError, NetworkErrorKind, ResolvedApiKey};
///
/// let key = ResolvedApiKey::try_new(BackendKeyId::OpenAi, "sk-test".to_owned())?;
/// let request = HttpRequest::new(
///     Method::Post,
///     Endpoint::parse("https://api.openai.com/v1/responses")?,
///     Arc::from("{}"),
/// )
/// .with_secret(SecretHeader::bearer(key));
/// let error = JevError::network(NetworkErrorKind::Connect, "echo: Bearer sk-test");
/// assert_eq!(
///     redact(error, &request).to_string(),
///     "network error (connect): echo: Bearer [redacted]"
/// );
/// # Ok::<(), JevError>(())
/// ```
#[must_use]
pub fn redact(error: JevError, request: &HttpRequest) -> JevError {
    match request.secret() {
        Some(secret) => secret.key().redact_error(error),
        None => error,
    }
}
