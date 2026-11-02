//! `JevClient` as a [`JudgeBackend`](sc_agent_types::JudgeBackend).
//!
//! Transport and the resolved API key are construction state. `judge` does
//! not accept either of them (ADR-SAG-0006).

mod https;
mod retry;
mod transport;

use std::fmt;
use std::sync::Arc;

use sc_agent_types::{BackendId, BackendKeyId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest};

use crate::api::JevClientConfig;
use crate::check::{self, PreparedCheck};

pub use https::HttpsTransport;
pub use retry::{RetryDelay, ThreadRetryDelay};
pub use transport::{JevTransport, WireRequest, WireResponse};

pub(crate) use retry::retry_delay;

const BUSY: [u16; 3] = [429, 502, 529];

/// TypeSafe check/judge client.
///
/// Posts `JudgeRequest::payload` bytes with the pinned model spliced in
/// front. Ignores `JudgeRequest::llm`. Retries HTTP 429, 502, and 529 with
/// the same body.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use sc_agent_types::{payload_bundle_from_value, JudgeBackend, JudgeRequest};
/// use sc_jev::{JevClient, JevTransport, WireRequest, WireResponse};
///
/// struct Scripted;
///
/// impl JevTransport for Scripted {
///     fn send(
///         &self,
///         request: &WireRequest<'_>,
///     ) -> Result<WireResponse, sc_agent_types::JevError> {
///         assert!(request.body.starts_with(br#"{"model":"jev-1.13.0","#));
///         assert!(request.authorization.starts_with("Bearer "));
///         Ok(WireResponse {
///             status: 200,
///             body: br#"{"model":"jev-1.13.0","answers":{"q":{"choice":"yes","confidence":1,"probabilities":{"no":0,"yes":1}}}}"#.to_vec(),
///             retry_after: None,
///         })
///     }
/// }
///
/// let payload = payload_bundle_from_value(&serde_json::json!({
///     "questions": {
///         "q": {
///             "type": "choice",
///             "instructions": "Is it?",
///             "criteria": {"no": "n", "yes": "y"}
///         }
///     },
///     "state": {}
/// }))
/// .expect("payload");
/// let client = JevClient::new("test-key", Arc::new(Scripted)).expect("key");
/// let outcome = client
///     .judge(&JudgeRequest { payload, llm: None })
///     .expect("judge");
/// assert_eq!(outcome.answers["q"]["choice"], "yes");
/// ```
pub struct JevClient {
    config: JevClientConfig,
    api_key: ApiKey,
    transport: Arc<dyn JevTransport>,
    retry_delay: Arc<dyn RetryDelay>,
}

impl JevClient {
    /// Creates a client for the packaged TypeSafe endpoint.
    ///
    /// `api_key` is the resolved credential. An empty key fails before any
    /// request can be sent.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when `api_key` is empty, and
    /// [`JevError::ConfigInvalid`] when it contains whitespace or a control
    /// character.
    pub fn new(
        api_key: impl AsRef<str>,
        transport: Arc<dyn JevTransport>,
    ) -> Result<Self, JevError> {
        Self::with_config(JevClientConfig::packaged(), api_key, transport)
    }

    /// Creates a client with explicit endpoint and retry settings.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when `api_key` is empty, and
    /// [`JevError::ConfigInvalid`] when it contains whitespace or a control
    /// character.
    pub fn with_config(
        config: JevClientConfig,
        api_key: impl AsRef<str>,
        transport: Arc<dyn JevTransport>,
    ) -> Result<Self, JevError> {
        Ok(Self {
            config,
            api_key: ApiKey::parse(api_key.as_ref())?,
            transport,
            retry_delay: Arc::new(ThreadRetryDelay),
        })
    }

    /// Replaces the busy-retry wait.
    ///
    /// Tests pass a recorder. The default waits on the calling thread.
    #[must_use]
    pub fn with_retry_delay(mut self, delay: Arc<dyn RetryDelay>) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Endpoint, model, and limits used for every `judge` call.
    #[must_use]
    pub fn config(&self) -> &JevClientConfig {
        &self.config
    }

    fn authorization(&self) -> String {
        format!("Bearer {}", self.api_key.as_str())
    }

    fn execute(&self, prepared: &PreparedCheck) -> Result<JudgeOutcome, JevError> {
        let url = self.config.endpoint();
        let authorization = self.authorization();
        let attempts = self.config.attempts();
        for attempt in 1..=attempts {
            let request = WireRequest {
                url: &url,
                body: &prepared.body,
                authorization: &authorization,
                timeout: self.config.timeout(),
            };
            let response = match self.transport.send(&request) {
                Ok(response) => response,
                Err(error) => return Err(redact_error(error, self.api_key.as_str())),
            };
            if BUSY.contains(&response.status) && attempt < attempts {
                self.retry_delay
                    .wait(retry_delay(response.retry_after.as_deref()));
                continue;
            }
            if response.status != 200 {
                return Err(JevError::NetworkError {
                    detail: format!("Jev HTTP {}", response.status),
                });
            }
            return check::parse_outcome(&response.body, self.config.model(), &prepared.criteria)
                .map_err(|error| redact_error(error, self.api_key.as_str()));
        }
        Err(JevError::NetworkError {
            detail: "Jev busy; attempts exhausted".to_owned(),
        })
    }
}

impl JudgeBackend for JevClient {
    fn id(&self) -> BackendId {
        BackendId::Jev
    }

    fn validate_for_send(&self, req: &JudgeRequest) -> Result<(), JevError> {
        let _ = &req.llm;
        check::prepare(&self.config, &req.payload.bytes).map(|_| ())
    }

    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        let _ = &req.llm;
        let prepared = check::prepare(&self.config, &req.payload.bytes)?;
        self.execute(&prepared)
    }
}

impl fmt::Debug for JevClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JevClient")
            .field("config", &self.config)
            .field("api_key", &self.api_key)
            .finish_non_exhaustive()
    }
}

struct ApiKey(String);

impl ApiKey {
    fn parse(raw: &str) -> Result<Self, JevError> {
        let key = raw.trim();
        if key.is_empty() {
            return Err(JevError::KeyMissing {
                backend: BackendKeyId::TypeSafe,
            });
        }
        if key
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        {
            return Err(JevError::ConfigInvalid {
                detail: "TypeSafe API key contains whitespace or a control character".to_owned(),
            });
        }
        Ok(Self(key.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey([redacted])")
    }
}

fn redact_error(error: JevError, key: &str) -> JevError {
    if key.is_empty() {
        return error;
    }
    match error {
        JevError::NetworkError { detail } => JevError::NetworkError {
            detail: detail.replace(key, "[redacted]"),
        },
        JevError::ResponseInvalid { detail } => JevError::ResponseInvalid {
            detail: detail.replace(key, "[redacted]"),
        },
        JevError::InputInvalid { detail } => JevError::InputInvalid {
            detail: detail.replace(key, "[redacted]"),
        },
        JevError::ConfigInvalid { detail } => JevError::ConfigInvalid {
            detail: detail.replace(key, "[redacted]"),
        },
        other => other,
    }
}
