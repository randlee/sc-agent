//! [`LlmError`], the one error type of sc-llm-provider (REQ-SAG-0002).

use std::error::Error;
use std::fmt;

use sc_agent_http::{HttpError, ProviderReply};
use sc_agent_types::{ErrorDetail, ErrorId};

use crate::LlmProvider;

/// Every error a public sc-llm-provider function returns.
///
/// `Display` writes `"<id>: <message>"` only. [`Self::Http`] keeps the [`HttpError`] whole:
/// it is [`Error::source`] and its detail is [`ErrorId::cause_detail`]. No variant carries a
/// key or a request body. Provider-written text ([`Self::ApiStatus`]'s reply, read with
/// [`ProviderReply::read`], and [`Self::Refused`]'s message) is redacted of the request's key
/// and otherwise kept whole (REQ-SAG-0002); every other message is this crate's own or a
/// serde_json message, never reply body bytes.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ErrorId;
/// use sc_llm_provider::{LlmError, LlmProvider};
///
/// let error = LlmError::Truncated { provider: LlmProvider::Anthropic, stop_reason: "max_tokens".to_owned() };
/// assert_eq!(error.id(), "llm.truncated");
/// assert_eq!(error.to_string(), format!("llm.truncated: {}", error.message()));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum LlmError {
    /// The send failed in sc-agent-http; the HttpError is kept whole (`llm.http`).
    Http {
        /// The provider the request was for.
        provider: LlmProvider,
        /// The send failure, unchanged.
        source: HttpError,
    },
    /// A non-2xx final reply, in the provider's own words, redacted (`llm.api_status`).
    ApiStatus {
        /// The provider that answered.
        provider: LlmProvider,
        /// What it said (boxed, as sc-agent-http's `QuotaExhausted::reply`: it keeps the
        /// error small on the `Ok` path, clippy `result_large_err`).
        reply: Box<ProviderReply>,
    },
    /// The 2xx body is not JSON or not the provider's envelope (`llm.reply_invalid`).
    ReplyInvalid {
        /// The provider that answered.
        provider: LlmProvider,
        /// What is wrong (a serde message or the missing field), never body bytes.
        message: String,
    },
    /// The reply has no text content (`llm.no_text`).
    NoText {
        /// The provider that answered.
        provider: LlmProvider,
    },
    /// The reply stopped at the token cap: `stop_reason` `max_tokens` or `finish_reason`
    /// `length` (`llm.truncated`).
    Truncated {
        /// The provider that answered.
        provider: LlmProvider,
        /// The stop reason, verbatim.
        stop_reason: String,
    },
    /// The model refused (`llm.refused`).
    Refused {
        /// The provider that answered.
        provider: LlmProvider,
        /// The model's refusal text, redacted of the key and otherwise whole, or a note
        /// naming the stop reason when the model gave none.
        message: String,
    },
    /// [`Output::Json`](crate::Output::Json) was asked for and the text does not parse
    /// (`llm.output_not_json`).
    OutputNotJson {
        /// The provider that answered.
        provider: LlmProvider,
        /// The serde_json message.
        message: String,
    },
}

impl ErrorId for LlmError {
    fn id(&self) -> &'static str {
        match self {
            Self::Http { .. } => "llm.http",
            Self::ApiStatus { .. } => "llm.api_status",
            Self::ReplyInvalid { .. } => "llm.reply_invalid",
            Self::NoText { .. } => "llm.no_text",
            Self::Truncated { .. } => "llm.truncated",
            Self::Refused { .. } => "llm.refused",
            Self::OutputNotJson { .. } => "llm.output_not_json",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Http { provider, source } => {
                format!("the {provider} send failed ({})", source.id())
            }
            Self::ApiStatus { provider, reply } => format!("{provider} answered {reply}"),
            Self::ReplyInvalid { provider, message } => {
                format!("the {provider} reply is not its documented envelope: {message}")
            }
            Self::NoText { provider } => format!("the {provider} reply has no text"),
            Self::Truncated {
                provider,
                stop_reason,
            } => format!("the {provider} reply was cut at the token cap ({stop_reason})"),
            Self::Refused { provider, message } => {
                format!("the {provider} model refused: {message}")
            }
            Self::OutputNotJson { provider, message } => {
                format!("the {provider} reply text is not JSON: {message}")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Http { source, .. } => Some(source.detail()),
            Self::ApiStatus { .. }
            | Self::ReplyInvalid { .. }
            | Self::NoText { .. }
            | Self::Truncated { .. }
            | Self::Refused { .. }
            | Self::OutputNotJson { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::Http { source, .. } => source.recovery(),
            Self::Truncated { .. } => Some("raise max_tokens or ask for a shorter reply"),
            Self::ApiStatus { .. } => Some(
                "read the provider's message: fix the key, model or request it names, or check the provider's status",
            ),
            Self::NoText { .. } => Some("send the request again; if it repeats, try another model"),
            Self::Refused { .. } => {
                Some("the model declined this request; rephrase it or use another model")
            }
            Self::OutputNotJson { .. } => Some(
                "send the request again; if it repeats, use a model that supports structured output",
            ),
            Self::ReplyInvalid { .. } => None,
        }
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for LlmError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use sc_agent_http::NetworkKind;

    use super::*;

    #[test]
    fn detail_is_built_from_id_message_recovery_and_cause() {
        let errors = [
            LlmError::Http {
                provider: LlmProvider::Anthropic,
                source: HttpError::network(NetworkKind::Connect, 1, "refused"),
            },
            LlmError::ApiStatus {
                provider: LlmProvider::Together,
                reply: Box::new(ProviderReply::read(
                    &sc_agent_http::HttpResponse::new(500, Vec::new(), b"down".to_vec()),
                    None,
                )),
            },
            LlmError::Refused {
                provider: LlmProvider::Anthropic,
                message: "no".to_owned(),
            },
            LlmError::NoText {
                provider: LlmProvider::Nous,
            },
            LlmError::Truncated {
                provider: LlmProvider::OpenAi,
                stop_reason: "length".to_owned(),
            },
        ];
        for error in errors {
            let detail = error.detail();
            assert_eq!(detail.id, error.id());
            assert_eq!(detail.message, error.message());
            assert_eq!(detail.recovery.as_deref(), error.recovery());
            assert_eq!(detail.cause.map(|cause| *cause), error.cause_detail());
            assert_eq!(
                error.to_string(),
                format!("{}: {}", error.id(), error.message())
            );
        }
    }

    #[test]
    fn http_keeps_the_send_error_as_source() {
        let source = HttpError::network(NetworkKind::Connect, 1, "refused");
        let error = LlmError::Http {
            provider: LlmProvider::OpenRouter,
            source: source.clone(),
        };
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(source.to_string())
        );
        assert_eq!(error.cause_detail(), Some(source.detail()));
    }
}
