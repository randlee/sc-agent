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
/// let error = DecisionLlmError::AnswerIds { missing: vec!["q2".to_owned()], extra: Vec::new() };
/// assert_eq!(error.id(), "decision_llm.answer_ids");
/// assert!(error.to_string().contains("q2"));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DecisionLlmError {
    /// The LLM call failed; the LlmError is kept whole (`decision_llm.llm`).
    Llm(LlmError),
    /// State, questions or schema could not be serialized (`decision_llm.encode`).
    Encode {
        /// The serde_json message.
        message: String,
    },
    /// The model's answer ids differ from the question ids (`decision_llm.answer_ids`).
    AnswerIds {
        /// Question ids with no answer, in request order.
        missing: Vec<String>,
        /// Answer ids that name no question, in reply order.
        extra: Vec<String>,
    },
    /// One answer lacks a required field, has a wrong type, or a choice outside the criteria
    /// (`decision_llm.answer_invalid`).
    AnswerInvalid {
        /// The question id; empty when the whole reply is at fault.
        question: String,
        /// What is wrong.
        message: String,
    },
}

impl From<LlmError> for DecisionLlmError {
    fn from(error: LlmError) -> Self {
        Self::Llm(error)
    }
}

impl ErrorId for DecisionLlmError {
    fn id(&self) -> &'static str {
        match self {
            Self::Llm(_) => "decision_llm.llm",
            Self::Encode { .. } => "decision_llm.encode",
            Self::AnswerIds { .. } => "decision_llm.answer_ids",
            Self::AnswerInvalid { .. } => "decision_llm.answer_invalid",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Llm(error) => format!("the LLM call failed ({})", error.id()),
            Self::Encode { message } => {
                format!("the decision request does not serialize: {message}")
            }
            Self::AnswerIds { missing, extra } => format!(
                "the answer ids differ from the question ids (missing: [{}]; extra: [{}])",
                missing.join(", "),
                extra.join(", ")
            ),
            Self::AnswerInvalid { question, message } if question.is_empty() => {
                format!("the reply is not the answer schema: {message}")
            }
            Self::AnswerInvalid { question, message } => {
                format!("answer `{question}` does not match its schema: {message}")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Llm(error) => Some(error.detail()),
            Self::Encode { .. } | Self::AnswerIds { .. } | Self::AnswerInvalid { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::Llm(error) => error.recovery(),
            Self::Encode { .. } | Self::AnswerIds { .. } | Self::AnswerInvalid { .. } => None,
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
