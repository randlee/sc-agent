//! [`LlmClient`]: sans-IO encode and decode of one LLM call, and [`Wire`].

use std::fmt;

use sc_agent_http::{Endpoint, HttpError, HttpRequest, HttpResponse, Method, ProviderReply};
use sc_agent_log::{GeneralLogger, Level, LogContext};
use sc_agent_secrets::ApiKey;
use serde_json::{Map, Value};

use crate::decode::{Envelope, decode_envelope, decode_model_ids};
use crate::endpoints::{call_url, models_url, secret_header, unsigned_headers};
use crate::{LlmError, LlmProvider, LlmRequest, LlmResponse, Output, encode};

/// The log target of this crate's records.
const LOG_TARGET: &str = "sc_llm_provider";

/// The most bytes of the provider-reported model name an `llm.usage` record carries.
const MAX_LOGGED_MODEL_BYTES: usize = 128;

/// `text` cut to at most `max` bytes at a char boundary, and whether it was cut.
fn bounded(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let end = (0..=max)
        .rev()
        .find(|index| text.is_char_boundary(*index))
        .unwrap_or(0);
    (text.get(..end).unwrap_or(""), true)
}

/// The HTTP dialect a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Wire {
    /// Anthropic Messages (`POST /v1/messages`).
    AnthropicMessages,
    /// OpenAI-compatible Chat Completions (`POST .../chat/completions`).
    ChatCompletions,
}

impl Wire {
    /// The wire of `provider`: Anthropic Messages for Anthropic, Chat Completions for the rest.
    #[must_use]
    pub const fn of(provider: LlmProvider) -> Self {
        match provider {
            LlmProvider::Anthropic => Self::AnthropicMessages,
            LlmProvider::OpenAi
            | LlmProvider::OpenRouter
            | LlmProvider::Together
            | LlmProvider::Nous => Self::ChatCompletions,
        }
    }
}

/// Encodes LLM calls to one provider and decodes its replies. Sends nothing.
///
/// `Debug` prints the provider, the wire, the endpoint's log form, `key: <redacted>` and
/// whether a logger is set; never the key.
///
/// # Examples
///
/// ```
/// # use sc_agent_secrets::{resolve_var, EnvSource, KeySources, KeyringSource, SecretsError};
/// # struct Env;
/// # impl EnvSource for Env { fn var(&self, _: &str) -> Option<String> { Some("sk-test".into()) } }
/// # struct NoKeyring;
/// # impl KeyringSource for NoKeyring {
/// #     fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> { Ok(None) }
/// # }
/// # let sources = KeySources { env: &Env, repo_root: None, keyring: &NoKeyring };
/// # let key = resolve_var(&"TEST_KEY".parse()?, &sources)?.key;
/// use sc_llm_provider::{LlmClient, LlmProvider, LlmRequest, Message, Output, Part, Role};
///
/// let client = LlmClient::new(LlmProvider::Anthropic, key)?;
/// let request = LlmRequest {
///     model: "claude-haiku-5-5".parse()?,
///     system: None,
///     messages: vec![Message { role: Role::User, content: vec![Part::Text("hi".to_owned())] }],
///     max_tokens: 64,
///     output: Output::Text,
/// };
/// let http = client.encode(&request)?;
/// assert_eq!(http.endpoint().as_str(), "https://api.anthropic.com/v1/messages");
/// assert!(!format!("{client:?} {http:?}").contains("sk-test"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct LlmClient {
    provider: LlmProvider,
    wire: Wire,
    endpoint: Endpoint,
    key: ApiKey,
    log: Option<GeneralLogger>,
}

impl fmt::Debug for LlmClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LlmClient")
            .field("provider", &self.provider)
            .field("wire", &self.wire)
            .field("endpoint", &self.endpoint.log_form())
            .field("key", &format_args!("<redacted>"))
            .field(
                "log",
                &format_args!(
                    "{}",
                    if self.log.is_some() {
                        "<set>"
                    } else {
                        "<unset>"
                    }
                ),
            )
            .finish()
    }
}

impl LlmClient {
    /// A client for `provider` at its built-in endpoint, with no logger.
    ///
    /// # Errors
    ///
    /// [`LlmError::Http`] with `http.invalid_endpoint` only if a built-in endpoint constant
    /// fails [`Endpoint::parse`]. Every built-in endpoint is a valid https URL, so this is
    /// unreachable in a released build: the `Result` is the plan's fixed signature and the
    /// boundary at which a bad constant would surface. The test
    /// `every_llm_provider_has_a_client` builds a client for every [`LlmProvider`], so a bad
    /// constant fails the test suite rather than a caller.
    pub fn new(provider: LlmProvider, key: ApiKey) -> Result<Self, LlmError> {
        let endpoint = Endpoint::parse(&call_url(provider))
            .map_err(|source| LlmError::Http { provider, source })?;
        Ok(Self {
            provider,
            wire: Wire::of(provider),
            endpoint,
            key,
            log: None,
        })
    }

    /// This client sending to `endpoint` instead of the built-in one (a gateway or a test).
    #[must_use]
    pub fn with_endpoint(self, endpoint: Endpoint) -> Self {
        Self { endpoint, ..self }
    }

    /// This client writing `llm.usage` records to `log`, the caller's general logger
    /// (default or injected, REQ-SAG-0008).
    #[must_use]
    pub fn with_log(self, log: GeneralLogger) -> Self {
        Self {
            log: Some(log),
            ..self
        }
    }

    /// The provider.
    #[must_use]
    pub fn provider(&self) -> LlmProvider {
        self.provider
    }

    /// The wire the provider speaks.
    #[must_use]
    pub fn wire(&self) -> Wire {
        self.wire
    }

    /// The POST that sends `request`: JSON body, wire headers and the credential header.
    ///
    /// The body's `model` is [`provider_model_id`](crate::provider_model_id) of
    /// `request.model` (REQ-SAG-0103). Sans-IO: nothing is sent.
    ///
    /// # Errors
    ///
    /// [`LlmError::Http`] when sc-agent-http refuses a header or the secret (`http.key_on_plain_http`
    /// can only come from a [`Self::with_endpoint`] override: every built-in endpoint is
    /// https). The body itself cannot fail: it is a `serde_json::Value`, whose keys are all
    /// strings, written with its infallible `Display`.
    pub fn encode(&self, request: &LlmRequest<'_>) -> Result<HttpRequest, LlmError> {
        let body = encode::body(self.provider, request)
            .to_string()
            .into_bytes();
        self.http_request(Method::Post, self.endpoint.clone(), body)
    }

    /// Decodes the final reply to `request`.
    ///
    /// Only `request.output` is read: [`Output::Json`] parses the text into
    /// [`LlmResponse::json`]. With a logger set, every 2xx reply whose envelope decodes
    /// (truncated, refused and not-JSON outputs included) writes one `llm.usage` record
    /// (provider, model as reported, cut to 128 bytes with `model_truncated: true` when
    /// longer, input and output tokens) carrying `ctx`. A log write
    /// error is ignored (sc-agent-log counts it) and never changes the result.
    ///
    /// # Errors
    ///
    /// - [`LlmError::ApiStatus`] for a non-2xx status, read with [`ProviderReply::read`] and
    ///   redacted of this client's key;
    /// - [`LlmError::ReplyInvalid`] when the body is not the wire's envelope;
    /// - [`LlmError::Refused`] (the model's text redacted of the key, otherwise whole), then
    ///   [`LlmError::Truncated`], then [`LlmError::NoText`];
    /// - [`LlmError::OutputNotJson`] when JSON was asked for and the text does not parse.
    pub fn decode(
        &self,
        request: &LlmRequest<'_>,
        response: HttpResponse,
        ctx: LogContext,
    ) -> Result<LlmResponse, LlmError> {
        self.check_status(&response)?;
        let envelope = decode_envelope(self.provider, self.wire, response.body())?;
        self.log_usage(&envelope, ctx);
        let provider = self.provider;
        let Envelope {
            model,
            text,
            stop_reason,
            refusal,
            truncated,
            usage,
        } = envelope;
        if let Some(refusal) = refusal {
            let message = if refusal.is_empty() {
                format!(
                    "no refusal text (stop reason {})",
                    stop_reason.as_deref().unwrap_or("none")
                )
            } else {
                // Provider-written text: kept whole (REQ-SAG-0002), with the key scrubbed as
                // ProviderReply::read scrubs an ApiStatus reply.
                self.key.redact_text(&refusal).into_owned()
            };
            return Err(LlmError::Refused { provider, message });
        }
        if truncated {
            return Err(LlmError::Truncated {
                provider,
                stop_reason: stop_reason.unwrap_or_default(),
            });
        }
        let text = text.ok_or(LlmError::NoText { provider })?;
        let json = match &request.output {
            Output::Text => None,
            Output::Json { .. } => {
                Some(
                    serde_json::from_str(&text).map_err(|error| LlmError::OutputNotJson {
                        provider,
                        message: error.to_string(),
                    })?,
                )
            }
        };
        Ok(LlmResponse {
            model,
            text,
            json,
            stop_reason,
            usage,
        })
    }

    /// `error`, a failure sending one of this client's requests, labelled with its provider.
    #[must_use]
    pub fn http_error(&self, error: HttpError) -> LlmError {
        LlmError::Http {
            provider: self.provider,
            source: error,
        }
    }

    /// `GET {base}/models` with the credential header, for decide doctor.
    ///
    /// The URL is the call endpoint with its `/messages` or `/chat/completions` suffix
    /// replaced by `/models` (`https://api.anthropic.com/v1/models`). The caller sends it on a
    /// probe transport (`UreqTransport::new(Timeouts::probe())`), not the decision one.
    ///
    /// # Errors
    ///
    /// [`LlmError::Http`] when the models URL does not parse or sc-agent-http refuses a
    /// header or the secret.
    pub fn models_request(&self) -> Result<HttpRequest, LlmError> {
        let url = models_url(self.endpoint.as_str(), self.wire);
        let endpoint = if self.endpoint.is_plain_http() {
            Endpoint::parse_local(&url)
        } else {
            Endpoint::parse(&url)
        }
        .map_err(|source| self.http_error(source))?;
        self.http_request(Method::Get, endpoint, Vec::new())
    }

    /// The provider's model ids, in the order listed.
    ///
    /// # Errors
    ///
    /// [`LlmError::ApiStatus`] for a non-2xx status; [`LlmError::ReplyInvalid`] when the body
    /// is neither `{"data":[{"id":..}]}` nor `[{"id":..}]`.
    pub fn decode_models(&self, response: HttpResponse) -> Result<Vec<String>, LlmError> {
        self.check_status(&response)?;
        decode_model_ids(self.provider, response.body())
    }

    /// Non-2xx -> [`LlmError::ApiStatus`], in the provider's words, redacted of the key.
    fn check_status(&self, response: &HttpResponse) -> Result<(), LlmError> {
        if (200..300).contains(&response.status()) {
            return Ok(());
        }
        Err(LlmError::ApiStatus {
            provider: self.provider,
            reply: Box::new(ProviderReply::read(response, Some(&self.key))),
        })
    }

    /// A request to `endpoint` with the wire headers and the credential header.
    fn http_request(
        &self,
        method: Method,
        endpoint: Endpoint,
        body: Vec<u8>,
    ) -> Result<HttpRequest, LlmError> {
        unsigned_headers(self.wire)
            .into_iter()
            .try_fold(
                HttpRequest::new(method, endpoint, body),
                |request, (name, value)| request.with_header(name, value),
            )
            .and_then(|request| request.with_secret(secret_header(self.wire, self.key.clone())))
            .map_err(|source| self.http_error(source))
    }

    /// One `llm.usage` record, when a logger is set. Never a prompt, body or key.
    fn log_usage(&self, envelope: &Envelope, ctx: LogContext) {
        let Some(log) = &self.log else {
            return;
        };
        let mut fields = Map::new();
        fields.insert("provider".to_owned(), self.provider.as_str().into());
        let (model, cut) = bounded(&envelope.model, MAX_LOGGED_MODEL_BYTES);
        fields.insert("model".to_owned(), model.into());
        if cut {
            fields.insert("model_truncated".to_owned(), true.into());
        }
        fields.insert(
            "input_tokens".to_owned(),
            Value::from(envelope.usage.input_tokens),
        );
        fields.insert(
            "output_tokens".to_owned(),
            Value::from(envelope.usage.output_tokens),
        );
        // A failed write is counted by sc-agent-log and reported by its flush; it never
        // changes a decode result.
        let _ = log.write(Level::Info, LOG_TARGET, "llm.usage", ctx, fields);
    }
}

#[cfg(test)]
mod tests {
    use super::bounded;

    #[test]
    fn bounded_cuts_at_a_char_boundary() {
        assert_eq!(bounded("short", 128), ("short", false));
        assert_eq!(bounded("abcdef", 3), ("abc", true));
        // 'é' is two bytes: a cut inside it backs off to the boundary before it.
        assert_eq!(bounded("aé", 2), ("a", true));
    }
}
