//! [`LlmDecisionBackend`]: Haiku and Luna as one [`Backend`].

use std::fmt;

use sc_agent_http::{HttpRequest, HttpResponse, LogContext};
use sc_agent_log::{GeneralLogger, Level};
use sc_agent_types::{ErrorId, ModelId};
use sc_decision_schema::{Backend, DecisionError, DecisionRequest, SystemOneResponse};
use sc_llm_provider::{LlmClient, LlmError, LlmRequest};
use serde_json::Map;

use crate::DecisionLlmError;
use crate::answers::answers;
use crate::prompt::{output, translate};

/// The backend name in every `DecisionError` this crate returns.
const BACKEND: &str = "llm";
/// The log target of this crate's records.
const LOG_TARGET: &str = "sc_decision_llm";

/// A decision backend that asks an LLM to answer the Jev questions itself.
///
/// Haiku and Luna are this one type with a different (provider, model):
///
/// - Haiku: `LlmClient::new(LlmProvider::Anthropic, key)` with
///   `catalog.resolve(&ModelChoice::Latest(ShortName::Haiku))`;
/// - Luna: `LlmClient::new(LlmProvider::OpenAi, key)` with
///   `catalog.resolve(&ModelChoice::Latest(ShortName::Luna))`.
///
/// `Debug` prints the client (never its key), the model, the token cap and whether a logger
/// is set.
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
/// use sc_agent_types::{Catalog, ModelChoice, ShortName};
/// use sc_decision_llm::LlmDecisionBackend;
/// use sc_decision_schema::{Backend, DecisionRequest, SystemOneRequest};
/// use sc_llm_provider::{LlmClient, LlmProvider};
///
/// let model = Catalog::builtin().resolve(&ModelChoice::Latest(ShortName::Haiku));
/// let haiku = LlmDecisionBackend::new(LlmClient::new(LlmProvider::Anthropic, key)?, model);
/// let body = SystemOneRequest::from_json(
///     r#"{"model":"haiku","state":"The sky is green.","questions":{"q1":{"type":"noul"}}}"#,
/// )?;
/// let http = haiku.encode(&DecisionRequest::new(body, Vec::new(), false)?)?;
/// assert_eq!(http.endpoint().as_str(), "https://api.anthropic.com/v1/messages");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct LlmDecisionBackend {
    llm: LlmClient,
    model: ModelId,
    max_tokens: u32,
    log: Option<GeneralLogger>,
}

impl fmt::Debug for LlmDecisionBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LlmDecisionBackend")
            .field("llm", &self.llm)
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
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

impl LlmDecisionBackend {
    /// The default completion token cap.
    pub const DEFAULT_MAX_TOKENS: u32 = 16_000;

    /// A backend asking `model` through `llm`, with [`Self::DEFAULT_MAX_TOKENS`] and no
    /// logger of its own.
    #[must_use]
    pub fn new(llm: LlmClient, model: ModelId) -> Self {
        Self {
            llm,
            model,
            max_tokens: Self::DEFAULT_MAX_TOKENS,
            log: None,
        }
    }

    /// This backend with completion token cap `max_tokens`.
    ///
    /// This is the only input limit set here. Image count and size are enforced once, in
    /// `sc_decision_schema::DecisionRequest::new` (A-2: `MAX_IMAGES` images,
    /// `MAX_IMAGE_BYTES` per image), so every request this backend encodes is already within
    /// them.
    ///
    /// # Errors
    ///
    /// [`DecisionLlmError::InvalidMaxTokens`] for 0: a provider would refuse it, or return
    /// an empty, truncated reply.
    pub fn with_max_tokens(self, max_tokens: u32) -> Result<Self, DecisionLlmError> {
        if max_tokens == 0 {
            return Err(DecisionLlmError::InvalidMaxTokens { max_tokens });
        }
        Ok(Self { max_tokens, ..self })
    }

    /// This backend and its [`LlmClient`] writing to `log`, so one decode writes `llm.usage`
    /// (the client) and, on a mismatch, `llm.schema_mismatch` (this backend).
    #[must_use]
    pub fn with_log(self, log: GeneralLogger) -> Self {
        Self {
            llm: self.llm.with_log(log.clone()),
            log: Some(log),
            ..self
        }
    }

    /// The catalog model this backend asks.
    #[must_use]
    pub fn model(&self) -> &ModelId {
        &self.model
    }

    /// The `DecisionError` of `error`: a schema mismatch (resent by the runner) or a backend
    /// failure, with the whole chain. A mismatch writes one `llm.schema_mismatch` record.
    fn decision_error(&self, error: DecisionLlmError, ctx: LogContext) -> DecisionError {
        if is_mismatch(&error) {
            self.log_mismatch(&error, ctx);
            DecisionError::SchemaMismatch {
                backend: BACKEND,
                detail: error.detail(),
            }
        } else {
            backend_error(&error)
        }
    }

    /// One `llm.schema_mismatch` record, when a logger is set. Never model text, a prompt,
    /// a body or a key. A write error is ignored (sc-agent-log counts it).
    fn log_mismatch(&self, error: &DecisionLlmError, ctx: LogContext) {
        let Some(log) = &self.log else {
            return;
        };
        let cause = match error {
            DecisionLlmError::Llm(inner) => inner.id(),
            other => other.id(),
        };
        let mut fields = Map::new();
        fields.insert("provider".to_owned(), self.llm.provider().as_str().into());
        fields.insert("model".to_owned(), self.model.as_str().into());
        fields.insert("error".to_owned(), cause.into());
        if let DecisionLlmError::AnswerInvalid { question, .. } = error
            && !question.is_empty()
        {
            fields.insert("question".to_owned(), question.clone().into());
        }
        let _ = log.write(Level::Info, LOG_TARGET, "llm.schema_mismatch", ctx, fields);
    }

    /// The request handed to [`LlmClient::decode`]: it reads only `output`, so the messages
    /// (and their images) are not rebuilt.
    fn decode_request(&self, request: &DecisionRequest) -> LlmRequest<'static> {
        LlmRequest {
            model: self.model.clone(),
            system: None,
            messages: Vec::new(),
            max_tokens: self.max_tokens,
            output: output(request),
        }
    }
}

/// Whether `error` means the reply did not match the required schema (REQ-SAG-0901).
fn is_mismatch(error: &DecisionLlmError) -> bool {
    match error {
        DecisionLlmError::AnswerIds { .. } | DecisionLlmError::AnswerInvalid { .. } => true,
        DecisionLlmError::Llm(inner) => matches!(
            inner,
            LlmError::OutputNotJson { .. } | LlmError::Truncated { .. } | LlmError::NoText { .. }
        ),
        DecisionLlmError::Encode { .. }
        | DecisionLlmError::InvalidMaxTokens { .. }
        | DecisionLlmError::Invariant { .. } => false,
    }
}

/// `decision.backend` with the whole chain; never resent.
fn backend_error(error: &DecisionLlmError) -> DecisionError {
    DecisionError::Backend {
        backend: BACKEND,
        detail: error.detail(),
    }
}

impl Backend for LlmDecisionBackend {
    /// `DecisionRequest` -> `LlmRequest` (built-in prompt, per-question JSON schema, every
    /// asset as an image part) -> `HttpRequest`.
    ///
    /// # Errors
    ///
    /// [`DecisionError::Backend`] (backend `"llm"`, never resent) carrying
    /// `decision_llm.encode` (a question type this backend does not know, an asset with no
    /// image, or state/questions that do not serialize), `decision_llm.invariant`, or the
    /// `LlmError` chain (`llm.http`). No supported input is refused here: a provider that
    /// rejects one returns its own error at decode.
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        let call = translate(request, &self.model, self.max_tokens)
            .map_err(|error| backend_error(&error))?;
        self.llm
            .encode(&call)
            .map_err(|error| backend_error(&DecisionLlmError::Llm(error)))
    }

    /// `HttpResponse` -> `LlmResponse` -> `SystemOneResponse` with one answer per question, in
    /// request order. `ctx` is passed to [`LlmClient::decode`], so `llm.usage` and
    /// `llm.schema_mismatch` carry it.
    ///
    /// # Errors
    ///
    /// - [`DecisionError::SchemaMismatch`] (backend `"llm"`) for `llm.output_not_json`,
    ///   `llm.truncated`, `llm.no_text`, `decision_llm.answer_ids` and
    ///   `decision_llm.answer_invalid`: the runner resends;
    /// - [`DecisionError::Backend`] for every other failure (`llm.api_status`,
    ///   `llm.reply_invalid`, `llm.refused`, ...): not resent.
    fn decode(
        &self,
        response: HttpResponse,
        request: &DecisionRequest,
        ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        let ctx = *ctx;
        let reply = self
            .llm
            .decode(&self.decode_request(request), response, ctx)
            .map_err(|error| self.decision_error(DecisionLlmError::Llm(error), ctx))?;
        answers(request.body(), reply).map_err(|error| self.decision_error(error, ctx))
    }
}

#[cfg(test)]
mod tests {
    use sc_agent_http::{HttpError, NetworkKind};
    use sc_agent_secrets::{EnvSource, KeySources, KeyringSource, SecretsError, resolve_var};
    use sc_llm_provider::LlmProvider;

    use super::*;

    struct OneVar;
    impl EnvSource for OneVar {
        fn var(&self, _: &str) -> Option<String> {
            Some("sk-decision-llm-unit".to_owned())
        }
    }
    struct NoKeyring;
    impl KeyringSource for NoKeyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            Ok(None)
        }
    }

    /// AC 13 (REQ-SAG-0002): an injected HttpError::Network reaches DecisionError's detail
    /// chain with `http.network` and its message.
    #[test]
    fn injected_network_error_reaches_the_decision_error_chain() {
        let sources = KeySources {
            env: &OneVar,
            repo_root: None,
            keyring: &NoKeyring,
        };
        let key = resolve_var(&"UNIT_KEY".parse().unwrap(), &sources)
            .unwrap()
            .key;
        let llm = LlmClient::new(LlmProvider::Anthropic, key).unwrap();
        let backend = LlmDecisionBackend::new(llm.clone(), "claude-haiku-5-5".parse().unwrap());
        let network = HttpError::network(NetworkKind::Connect, 1, "connection refused by peer");
        let error = backend.decision_error(
            DecisionLlmError::from(llm.http_error(network)),
            LogContext::NONE,
        );
        assert_eq!(error.id(), "decision.backend");
        let mut ids = Vec::new();
        let mut level = error.cause_detail();
        let mut network_message = None;
        while let Some(detail) = level {
            ids.push(detail.id.clone());
            if detail.id == "http.network" {
                network_message = Some(detail.message.clone());
            }
            level = detail.cause.map(|cause| *cause);
        }
        assert_eq!(ids, ["decision_llm.llm", "llm.http", "http.network"]);
        assert!(
            network_message
                .as_deref()
                .is_some_and(|message| message.contains("connection refused by peer")),
            "{network_message:?}"
        );
    }
}
