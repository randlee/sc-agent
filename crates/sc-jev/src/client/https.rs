//! Blocking HTTPS transport for the packaged TypeSafe endpoint.
//!
//! Each send uses `timeout_global` from the request, refuses cleartext, and
//! does not follow redirects. 4xx and 5xx stay in the [`crate::WireResponse`]
//! so the client can apply the busy-retry policy.

use std::time::Duration;

use sc_agent_types::JevError;
use ureq::Agent;

use super::{JevTransport, WireRequest, WireResponse};

/// Production transport. Holds no API key; the client passes it per send.
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
        let agent = self.agent(request.timeout);
        let mut response = agent
            .post(request.url)
            .header("Authorization", request.authorization)
            .header("Content-Type", "application/json")
            .send(request.body)
            .map_err(|error| network(&error, request.authorization))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response
            .body_mut()
            .read_to_vec()
            .map_err(|error| network(&error, request.authorization))?;
        Ok(WireResponse {
            status,
            body,
            retry_after,
        })
    }
}

fn network(error: &ureq::Error, authorization: &str) -> JevError {
    let mut detail = format!("Jev connection failed: {error}");
    if let Some(key) = authorization.strip_prefix("Bearer ")
        && !key.is_empty()
    {
        detail = detail.replace(key, "[redacted]");
    }
    detail = detail.replace(authorization, "[redacted]");
    JevError::NetworkError { detail }
}
