//! Haiku and Luna [`JudgeBackend`](sc_agent_types::JudgeBackend) implementations.
//!
//! Both backends count tokens, then send the judge message. The count request
//! uses the same body as the judge call except Haiku omits `max_tokens`.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use sc_agent_types::{BackendId, BackendKeyId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest};

use crate::retry::{self, RetrySleeper, ThreadSleeper};
use crate::transport::{
    BackendTransport, DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, HttpRequest, HttpResponse,
};
use crate::validate::{self, ParsedQuestion};
use crate::wire::{self, ProviderKind};

#[derive(Clone)]
struct Provider {
    id: BackendId,
    key_id: BackendKeyId,
    name: &'static str,
    model: &'static str,
    host: &'static str,
    message_path: &'static str,
    count_path: &'static str,
    kind: ProviderKind,
}

impl Provider {
    fn haiku() -> Self {
        Self {
            id: BackendId::AnthropicHaiku,
            key_id: BackendKeyId::Anthropic,
            name: "haiku",
            model: wire::HAIKU_MODEL,
            host: wire::ANTHROPIC_HOST,
            message_path: wire::HAIKU_MESSAGE_PATH,
            count_path: wire::HAIKU_COUNT_PATH,
            kind: ProviderKind::Anthropic,
        }
    }

    fn luna() -> Self {
        Self {
            id: BackendId::OpenAiLuna,
            key_id: BackendKeyId::OpenAi,
            name: "luna",
            model: wire::LUNA_MODEL,
            host: wire::OPENAI_HOST,
            message_path: wire::LUNA_MESSAGE_PATH,
            count_path: wire::LUNA_COUNT_PATH,
            kind: ProviderKind::OpenAi,
        }
    }
}

struct Prepared {
    questions: Vec<ParsedQuestion>,
    message_body: Vec<u8>,
    count_body: Vec<u8>,
}

#[derive(Clone)]
struct Client {
    provider: Provider,
    api_key: String,
    transport: Arc<dyn BackendTransport>,
    sleeper: Arc<dyn RetrySleeper>,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl Client {
    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        let _ = self.prepare(request)?;
        Ok(())
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        let prepared = self.prepare(request)?;
        let counted = self.send(self.provider.count_path, &prepared.count_body)?;
        let prompt_tokens = wire::parse_count(self.provider.name, &counted.body)?;
        let judged = self.send(self.provider.message_path, &prepared.message_body)?;
        let reply = wire::parse_vendor(self.provider.kind, self.provider.name, &judged.body)?;
        if let Some(reason) = &reply.truncation {
            return Err(JevError::ResponseInvalid {
                detail: reason.clone(),
            });
        }
        let answers = wire::parse_answers(&reply.text, &prepared.questions)?;
        Ok(JudgeOutcome {
            answers,
            backend: self.provider.id,
            probabilities: None,
            provider_meta: Some(wire::provider_meta(
                &reply,
                self.provider.model,
                prompt_tokens,
            )),
        })
    }

    fn prepare(&self, request: &JudgeRequest) -> Result<Prepared, JevError> {
        let prompt = request.llm.as_ref().ok_or_else(|| JevError::InputInvalid {
            detail: "LLM system prompt is required".into(),
        })?;
        let payload_text =
            std::str::from_utf8(&request.payload.bytes).map_err(|_| JevError::InputInvalid {
                detail: "canonical payload is not UTF-8".into(),
            })?;
        let questions = validate::validate_questions(&request.payload.bytes)?;
        let message_body = wire::message_body(
            self.provider.kind,
            self.provider.model,
            payload_text,
            &prompt.system_prompt,
        );
        if message_body.len() as u64 > validate::MAX_REQUEST_BYTES {
            return Err(validate::too_large(message_body.len() as u64));
        }
        let count_body = wire::count_body(self.provider.kind, &message_body);
        Ok(Prepared {
            questions,
            message_body,
            count_body,
        })
    }

    fn send(&self, path: &str, body: &[u8]) -> Result<HttpResponse, JevError> {
        let mut attempt = 0_u32;
        loop {
            attempt += 1;
            let response = self
                .transport
                .send(self.http_request(path, body))
                .map_err(|error| redact_error(error, &self.api_key))?;
            if response.status == 200 {
                return Ok(response);
            }
            if retry::is_busy(response.status) && attempt < retry::BUSY_ATTEMPTS {
                self.sleeper.wait(retry::retry_delay(&response));
                continue;
            }
            let error = if retry::is_busy(response.status) {
                retry::exhausted(self.provider.name, &response)
            } else {
                retry::http_failure(self.provider.name, &response)
            };
            return Err(redact_error(error, &self.api_key));
        }
    }

    fn http_request(&self, path: &str, body: &[u8]) -> HttpRequest {
        HttpRequest {
            method: "POST".to_owned(),
            url: format!("https://{}{path}", self.provider.host),
            headers: wire::headers(self.provider.kind, &self.api_key),
            body: body.to_vec(),
            connect_timeout: self.connect_timeout,
            request_timeout: self.request_timeout,
        }
    }
}

fn redact_error(error: JevError, secret: &str) -> JevError {
    match error {
        JevError::NetworkError { detail } => JevError::NetworkError {
            detail: redact(&detail, secret),
        },
        JevError::ResponseInvalid { detail } => JevError::ResponseInvalid {
            detail: redact(&detail, secret),
        },
        other => other,
    }
}

fn redact(detail: &str, secret: &str) -> String {
    if secret.is_empty() {
        return detail.to_owned();
    }
    detail.replace(secret, "[redacted]")
}

fn normalize_key(raw: String, backend: BackendKeyId) -> Result<String, JevError> {
    let key = raw.trim().to_owned();
    if key.is_empty() || key.chars().any(char::is_control) {
        return Err(JevError::KeyMissing { backend });
    }
    Ok(key)
}

fn assemble(
    provider: Provider,
    api_key: String,
    transport: impl BackendTransport + 'static,
    sleeper: Arc<dyn RetrySleeper>,
) -> Result<Arc<Client>, JevError> {
    let key_id = provider.key_id;
    Ok(Arc::new(Client {
        api_key: normalize_key(api_key, key_id)?,
        provider,
        transport: Arc::new(transport),
        sleeper,
        connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        request_timeout: DEFAULT_REQUEST_TIMEOUT,
    }))
}

/// API-direct Anthropic Haiku judge.
///
/// # Examples
///
/// ```
/// use sc_agent_types::JevError;
/// use sc_jev_backends::{
///     AnthropicHaikuBackend, BackendTransport, HttpRequest, HttpResponse,
/// };
///
/// struct Offline;
///
/// impl BackendTransport for Offline {
///     fn send(&self, _request: HttpRequest) -> Result<HttpResponse, JevError> {
///         Err(JevError::NetworkError {
///             detail: "offline".into(),
///         })
///     }
/// }
///
/// let error = AnthropicHaikuBackend::new("  ", Offline).expect_err("blank key");
/// assert_eq!(error.code(), "KEY_MISSING");
/// ```
#[derive(Clone)]
pub struct AnthropicHaikuBackend {
    inner: Arc<Client>,
}

impl AnthropicHaikuBackend {
    /// Builds a Haiku backend with the default timeouts.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when `api_key` is empty or contains a
    /// control character.
    pub fn new(
        api_key: impl Into<String>,
        transport: impl BackendTransport + 'static,
    ) -> Result<Self, JevError> {
        Ok(Self {
            inner: assemble(
                Provider::haiku(),
                api_key.into(),
                transport,
                Arc::new(ThreadSleeper),
            )?,
        })
    }

    /// Replaces the wait used between busy retries of the same body.
    ///
    /// The default wait blocks the calling thread. Callers that must not block,
    /// including tests, pass their own [`RetrySleeper`].
    #[must_use]
    pub fn with_retry_sleeper(mut self, sleeper: impl RetrySleeper + 'static) -> Self {
        Arc::make_mut(&mut self.inner).sleeper = Arc::new(sleeper);
        self
    }
}

impl fmt::Debug for AnthropicHaikuBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicHaikuBackend")
            .field("model", &self.inner.provider.model)
            .field("host", &self.inner.provider.host)
            .finish_non_exhaustive()
    }
}

impl JudgeBackend for AnthropicHaikuBackend {
    fn id(&self) -> BackendId {
        self.inner.provider.id
    }

    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        self.inner.validate_for_send(request)
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.inner.judge(request)
    }
}

/// API-direct OpenAI Luna judge.
#[derive(Clone)]
pub struct OpenAiLunaBackend {
    inner: Arc<Client>,
}

impl OpenAiLunaBackend {
    /// Builds a Luna backend with the default timeouts.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when `api_key` is empty or contains a
    /// control character.
    pub fn new(
        api_key: impl Into<String>,
        transport: impl BackendTransport + 'static,
    ) -> Result<Self, JevError> {
        Ok(Self {
            inner: assemble(
                Provider::luna(),
                api_key.into(),
                transport,
                Arc::new(ThreadSleeper),
            )?,
        })
    }

    /// Replaces the wait used between busy retries of the same body.
    ///
    /// The default wait blocks the calling thread. Callers that must not block,
    /// including tests, pass their own [`RetrySleeper`].
    #[must_use]
    pub fn with_retry_sleeper(mut self, sleeper: impl RetrySleeper + 'static) -> Self {
        Arc::make_mut(&mut self.inner).sleeper = Arc::new(sleeper);
        self
    }
}

impl fmt::Debug for OpenAiLunaBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiLunaBackend")
            .field("model", &self.inner.provider.model)
            .field("host", &self.inner.provider.host)
            .finish_non_exhaustive()
    }
}

impl JudgeBackend for OpenAiLunaBackend {
    fn id(&self) -> BackendId {
        self.inner.provider.id
    }

    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        self.inner.validate_for_send(request)
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.inner.judge(request)
    }
}
