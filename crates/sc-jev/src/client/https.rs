//! Blocking HTTPS transport for the packaged TypeSafe endpoint.
//!
//! Each send uses `timeout_global` from the request, refuses cleartext, and
//! does not follow redirects. The response body is capped at
//! [`MAX_RESPONSE_BYTES`]. 4xx and 5xx stay in the [`crate::WireResponse`]
//! so the client can apply the busy-retry policy.

use std::io::Read;
use std::time::Duration;

use sc_agent_types::JevError;
use ureq::Agent;

use super::{JevTransport, WireRequest, WireResponse};

/// Largest Jev response body this client will buffer.
///
/// A normal check reply fits well under this. Anything larger is
/// [`JevError::ResponseInvalid`] so a response cannot grow without a bound.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// Production transport. Holds no API key; each [`WireRequest`] carries it.
///
/// [`JevTransport::send`] is the only place this type reads the key.
#[derive(Debug, Clone)]
pub struct HttpsTransport {
    user_agent: String,
}

impl HttpsTransport {
    /// Builds a transport that speaks HTTPS only.
    #[must_use]
    pub fn new() -> Self {
        Self {
            user_agent: "sc-jev/0.1.0".to_owned(),
        }
    }

    fn agent(&self, timeout: Duration) -> Agent {
        Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .https_only(true)
            .max_redirects(0)
            .user_agent(self.user_agent.as_str())
            .build()
            .into()
    }
}

impl Default for HttpsTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl JevTransport for HttpsTransport {
    fn send(&self, request: &WireRequest<'_>) -> Result<WireResponse, JevError> {
        if !request.url.starts_with("https://") {
            return Err(JevError::NetworkError {
                detail: "Jev connection failed: URL is not https".to_owned(),
            });
        }
        let authorization = format!("Bearer {}", request.api_key.as_str());
        let agent = self.agent(request.timeout);
        let mut response = agent
            .post(request.url)
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json")
            .send(request.body)
            .map_err(|error| network(&error, &authorization))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut reader = response
            .body_mut()
            .as_reader()
            .take(MAX_RESPONSE_BYTES.saturating_add(1));
        let mut body = Vec::new();
        reader
            .read_to_end(&mut body)
            .map_err(|error| network_io(&error, &authorization))?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(JevError::ResponseInvalid {
                detail: redact(
                    format!("Jev response exceeds {MAX_RESPONSE_BYTES} bytes"),
                    &authorization,
                ),
            });
        }
        Ok(WireResponse {
            status,
            body,
            retry_after,
        })
    }
}

fn network(error: &ureq::Error, authorization: &str) -> JevError {
    JevError::NetworkError {
        detail: redact(format!("Jev connection failed: {error}"), authorization),
    }
}

fn network_io(error: &std::io::Error, authorization: &str) -> JevError {
    JevError::NetworkError {
        detail: redact(format!("Jev connection failed: {error}"), authorization),
    }
}

fn redact(mut detail: String, authorization: &str) -> String {
    if let Some(key) = authorization.strip_prefix("Bearer ")
        && !key.is_empty()
    {
        detail = detail.replace(key, "[redacted]");
    }
    detail.replace(authorization, "[redacted]")
}
