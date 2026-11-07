//! `JevClient` as a [`JudgeBackend`].
//!
//! Transport and the resolved API key are construction state. `judge` does
//! not accept either of them (ADR-SAG-0006).

mod https;
mod retry;
mod transport;

use std::fmt;
use std::sync::Arc;

use sc_agent_types::{
    BackendId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest, ResolvedApiKey,
};

use crate::check::{self, PreparedCheck};
use crate::config::JevClientConfig;

pub use https::HttpsTransport;
pub use retry::{RetryDelay, ThreadRetryDelay};
pub use transport::{JevTransport, WireRequest, WireResponse};

use crate::retry_policy::{self, RetryBudget};

/// TypeSafe check/judge client.
///
/// Posts `JudgeRequest::payload` bytes with the pinned model spliced in
/// front. Ignores `JudgeRequest::llm`. Retries HTTP 429, 502, and 529 with
/// the same body, honoring `Retry-After` (0 to 30 s plus jitter) within a
/// 90 s budget from the first send.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use sc_agent_types::{
///     payload_bundle_from_value, BackendKeyId, JudgeBackend, JudgeRequest, ResolvedApiKey,
/// };
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
/// let client = JevClient::new(
///     ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "test-key".to_owned()).expect("key"),
///     Arc::new(Scripted),
/// )
/// .expect("key");
/// let outcome = client
///     .judge(&JudgeRequest { payload, llm: None })
///     .expect("judge");
/// assert_eq!(outcome.answers["q"]["choice"], "yes");
/// ```
pub struct JevClient {
    config: JevClientConfig,
    api_key: ResolvedApiKey,
    transport: Arc<dyn JevTransport>,
    retry_delay: Arc<dyn RetryDelay>,
}

impl JevClient {
    /// Creates a client for the packaged TypeSafe endpoint.
    ///
    /// `api_key` is the resolved credential, already normalized by
    /// [`ResolvedApiKey::try_new`]. The client does not read it.
    /// [`JevTransport::send`] formats the `Bearer` header (ADR-SAG-0020).
    ///
    /// # Errors
    ///
    /// Does not fail today: key validation happens once, in
    /// [`ResolvedApiKey::try_new`]. The `Result` is kept so construction
    /// checks can be added without an API break.
    pub fn new(
        api_key: ResolvedApiKey,
        transport: Arc<dyn JevTransport>,
    ) -> Result<Self, JevError> {
        Self::with_config(JevClientConfig::packaged(), api_key, transport)
    }

    /// Creates a client with explicit endpoint and retry settings.
    ///
    /// # Errors
    ///
    /// Does not fail today; see [`Self::new`].
    pub fn with_config(
        config: JevClientConfig,
        api_key: ResolvedApiKey,
        transport: Arc<dyn JevTransport>,
    ) -> Result<Self, JevError> {
        Ok(Self {
            config,
            api_key,
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

    fn execute(&self, prepared: &PreparedCheck) -> Result<JudgeOutcome, JevError> {
        let url = self.config.endpoint();
        let attempts = self.config.attempts();
        let request = WireRequest {
            url: &url,
            body: &prepared.body,
            api_key: &self.api_key,
            timeout: self.config.timeout(),
        };
        let budget = RetryBudget::start();
        let mut sends = 0_u32;
        loop {
            sends += 1;
            let response = self
                .transport
                .send(&request)
                .map_err(|error| self.api_key.redact_error(error))?;
            if retry_policy::is_busy(response.status) {
                let wait = (sends < attempts)
                    .then(|| {
                        budget.next_wait(retry_policy::retry_after(response.retry_after.as_deref()))
                    })
                    .flatten();
                if let Some(wait) = wait {
                    self.retry_delay.wait(wait);
                    continue;
                }
                return Err(JevError::NetworkError {
                    detail: retry_policy::exhausted_detail("Jev", response.status, sends),
                });
            }
            if response.status != 200 {
                return Err(JevError::NetworkError {
                    detail: format!("Jev HTTP {}", response.status),
                });
            }
            return check::parse_outcome(&response.body, self.config.model(), &prepared.criteria)
                .map_err(|error| self.api_key.redact_error(error));
        }
    }
}

impl JudgeBackend for JevClient {
    fn id(&self) -> BackendId {
        BackendId::Jev
    }

    fn validate_for_send(&self, req: &JudgeRequest) -> Result<(), JevError> {
        check::prepare(&self.config, req.payload.bytes()).map(|_| ())
    }

    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        let prepared = check::prepare(&self.config, req.payload.bytes())?;
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
