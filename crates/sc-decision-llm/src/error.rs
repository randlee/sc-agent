//! [`DecisionLlmError`], the error type of sc-decision-llm (REQ-SAG-0002).

use std::error::Error;
use std::fmt;

use sc_agent_types::{ErrorDetail, ErrorId};
use sc_llm_provider::LlmError;

/// What went wrong translating a decision to or from an LLM call.
///
/// `Display` writes `"<id>: <message>"` only. [`Self::Llm`] keeps the [`LlmError`] whole (and
/// through it any `HttpError`): it is [`Error::source`] and its detail is
/// [`ErrorId::cause_detail`]. It reaches `DecisionError` as an `ErrorDetail` chain
/// (ADR-SAG-0004). No variant carries model text beyond the ids it names.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ErrorId;
/// use sc_decision_llm::DecisionLlmError;
///
/// let error = DecisionLlmError::AnswerIds { missing: vec!["q2".to_owned()], extra: Vec::new(), extra_count: 0 };
/// assert_eq!(error.id(), "decision_llm.answer_ids");
/// assert!(error.to_string().contains("q2"));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DecisionLlmError {
    /// The LLM call failed; the LlmError is kept whole (`decision_llm.llm`).
    Llm(LlmError),
    /// The request cannot be sent as an LLM call (`decision_llm.encode`): the state or
    /// questions do not serialize, a question has a type this backend does not know, or an
    /// asset carries no image (neither `data_base64` nor a part). Not a schema mismatch.
    Encode {
        /// What cannot be sent.
        message: String,
    },
    /// The model's answer ids differ from the question ids (`decision_llm.answer_ids`).
    AnswerIds {
        /// Question ids with no answer, in request order.
        missing: Vec<String>,
        /// Answer ids that name no question, in reply order: model-written, so at most
        /// [`Self::MAX_LISTED_IDS`] are kept, each cut to [`Self::MAX_MODEL_TEXT_BYTES`].
        extra: Vec<String>,
        /// How many answer ids name no question in all (may exceed `extra.len()`).
        extra_count: usize,
    },
    /// One answer lacks a required field, has a wrong type, or a choice outside the criteria
    /// (`decision_llm.answer_invalid`).
    AnswerInvalid {
        /// The question id; empty when the whole reply is at fault.
        question: String,
        /// What is wrong; any model-written name or value in it is cut to
        /// [`Self::MAX_MODEL_TEXT_BYTES`].
        message: String,
    },
    /// [`LlmDecisionBackend::with_max_tokens`](crate::LlmDecisionBackend::with_max_tokens)
    /// was given 0 (`decision_llm.invalid_max_tokens`).
    InvalidMaxTokens {
        /// The refused value.
        max_tokens: u32,
    },
    /// An invariant of this crate failed: a bug, never a property of the reply
    /// (`decision_llm.invariant`). Not a schema mismatch, so it is never resent.
    Invariant {
        /// Which invariant.
        message: String,
    },
}

impl DecisionLlmError {
    /// The most bytes of one model-written string (a field name, choice or id) an error
    /// message carries; longer text is cut at a char boundary and ends in `…`.
    pub const MAX_MODEL_TEXT_BYTES: usize = 64;

    /// The most extra answer ids [`Self::AnswerIds`] lists.
    pub const MAX_LISTED_IDS: usize = 8;
}

/// `text` cut to [`DecisionLlmError::MAX_MODEL_TEXT_BYTES`] at a char boundary, with `…` when
/// cut. For model-written text placed in this crate's own messages (RSH-002).
pub(crate) fn bounded(text: &str) -> String {
    let max = DecisionLlmError::MAX_MODEL_TEXT_BYTES;
    if text.len() <= max {
        return text.to_owned();
    }
    let end = (0..=max)
        .rev()
        .find(|index| text.is_char_boundary(*index))
        .unwrap_or(0);
    format!("{}…", text.get(..end).unwrap_or(""))
}

impl From<LlmError> for DecisionLlmError {
    fn from(error: LlmError) -> Self {
        Self::Llm(error)
    }
}

sc_agent_types::error_catalog! { DecisionLlmError {
    DecisionLlmError::Llm(_) => ("decision_llm.llm", None),
    DecisionLlmError::Encode { .. } => ("decision_llm.encode", Some(sc_agent_types::ErrorCategory::Other)),
    DecisionLlmError::AnswerIds { .. } => ("decision_llm.answer_ids", Some(sc_agent_types::ErrorCategory::ResponseInvalid)),
    DecisionLlmError::AnswerInvalid { .. } => ("decision_llm.answer_invalid", Some(sc_agent_types::ErrorCategory::ResponseInvalid)),
    DecisionLlmError::InvalidMaxTokens { .. } => ("decision_llm.invalid_max_tokens", Some(sc_agent_types::ErrorCategory::Validation)),
    DecisionLlmError::Invariant { .. } => ("decision_llm.invariant", Some(sc_agent_types::ErrorCategory::Other)),
} }

impl ErrorId for DecisionLlmError {
    fn id(&self) -> &'static str {
        self.catalog_row().0
    }
    fn base_category(&self) -> Option<sc_agent_types::ErrorCategory> {
        self.catalog_row().1
    }

    fn message(&self) -> String {
        match self {
            Self::Llm(error) => format!("the LLM call failed ({})", error.id()),
            Self::Encode { message } => {
                format!("the decision request cannot be sent as an LLM call: {message}")
            }
            Self::AnswerIds {
                missing,
                extra,
                extra_count,
            } => {
                let more = extra_count.saturating_sub(extra.len());
                let more = if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                };
                format!(
                    "the answer ids differ from the question ids (missing: [{}]; extra: [{}]{more})",
                    missing.join(", "),
                    extra.join(", ")
                )
            }
            Self::AnswerInvalid { question, message } if question.is_empty() => {
                format!("the reply is not the answer schema: {message}")
            }
            Self::AnswerInvalid { question, message } => {
                format!("answer `{question}` does not match its schema: {message}")
            }
            Self::InvalidMaxTokens { max_tokens } => {
                format!("max_tokens must be at least 1, not {max_tokens}")
            }
            Self::Invariant { message } => {
                format!("an internal invariant of sc-decision-llm failed: {message}")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Llm(error) => Some(error.detail()),
            Self::Encode { .. }
            | Self::AnswerIds { .. }
            | Self::AnswerInvalid { .. }
            | Self::InvalidMaxTokens { .. }
            | Self::Invariant { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::Llm(error) => error.recovery(),
            Self::AnswerIds { .. } => Some(
                "the runner resends the request; if it repeats, use a model that follows the required schema",
            ),
            Self::AnswerInvalid { .. } => Some(
                "the runner resends the request; if it repeats, use a model that follows the required schema",
            ),
            Self::InvalidMaxTokens { .. } => Some("give a max_tokens of 1 or more"),
            Self::Encode { .. } => Some(
                "fix the request named in the message: every question type must be noul, choice, score or box, and every asset must carry data_base64 or a part",
            ),
            Self::Invariant { .. } => Some("report this as an sc-decision-llm bug"),
        }
    }
}

impl fmt::Display for DecisionLlmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for DecisionLlmError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Llm(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_preserves_provider_auth_fields_and_causes() {
        let reply = sc_agent_http::ProviderReply::read(
            &sc_agent_http::HttpResponse::new(401, vec![], b"unauthorized".to_vec()),
            None,
        );
        let lower = LlmError::ApiStatus {
            provider: sc_llm_provider::LlmProvider::Anthropic,
            reply: Box::new(reply),
        };
        let lower_detail = lower.detail();
        let wrapped = DecisionLlmError::Llm(lower);
        let wrapped_detail = wrapped.detail();
        let top = sc_decision_schema::DecisionError::Backend {
            backend: "llm",
            detail: Box::new(wrapped_detail.clone()),
        }
        .detail();
        assert_eq!(
            (top.status, top.category),
            (Some(401), Some(sc_agent_types::ErrorCategory::Auth))
        );
        assert_eq!(top.cause.as_deref(), Some(&wrapped_detail));
        assert_eq!(wrapped_detail.id, "decision_llm.llm");
        assert_eq!(wrapped_detail.cause.as_deref(), Some(&lower_detail));
        assert_eq!(lower_detail.id, "llm.api_status");
    }

    /// a-7 REQ-SAG-0002: exactly 6 distinct `decision_llm.*` ids; the exhaustive match fails
    /// to compile when a variant is added or removed.
    #[test]
    fn there_are_six_decision_llm_ids() {
        let all = [
            DecisionLlmError::Llm(LlmError::NoText {
                provider: sc_llm_provider::LlmProvider::OpenAi,
            }),
            DecisionLlmError::Encode {
                message: "m".into(),
            },
            DecisionLlmError::AnswerIds {
                missing: vec!["q".into()],
                extra: Vec::new(),
                extra_count: 0,
            },
            DecisionLlmError::AnswerInvalid {
                question: "q".into(),
                message: "m".into(),
            },
            DecisionLlmError::InvalidMaxTokens { max_tokens: 0 },
            DecisionLlmError::Invariant {
                message: "m".into(),
            },
        ];
        for error in &all {
            match error {
                DecisionLlmError::Llm(_)
                | DecisionLlmError::Encode { .. }
                | DecisionLlmError::AnswerIds { .. }
                | DecisionLlmError::AnswerInvalid { .. }
                | DecisionLlmError::InvalidMaxTokens { .. }
                | DecisionLlmError::Invariant { .. } => {}
            }
        }
        let ids: std::collections::BTreeSet<&str> = all.iter().map(ErrorId::id).collect();
        sc_agent_testkit::assert_catalog_rows(&all);
        assert_eq!(ids.len(), 6, "{ids:?}");
        assert!(
            ids.iter().all(|id| id.starts_with("decision_llm.")),
            "{ids:?}"
        );
    }
}
