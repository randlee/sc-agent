#![forbid(unsafe_code)]
//! The Jev decision API: one request form for every backend, a typed answer per question type.

use indexmap::IndexMap;
use sc_agent_http::Codec;
use sc_agent_types::Error;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Question {
    /// Yes/no; the answer is the probability of yes.
    Noul {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<Value>,
    },
    /// Pick one of the named options.
    Choice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
        criteria: IndexMap<String, Value>,
    },
    /// Score on ordered levels (a level's number is its index).
    Score {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
        criteria: Vec<Value>,
    },
    /// Locate a region of an image.
    Box {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<Value>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Noul {
        noul: f64,
    },
    Choice {
        choice: String,
        confidence: f64,
        probabilities: IndexMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        #[serde(default)]
        legend: IndexMap<String, Value>,
        probabilities: IndexMap<String, f64>,
    },
    Box {
        #[serde(rename = "box")]
        region: Region,
        confidence: Option<f64>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub model: String,
    pub state: Value,
    pub questions: IndexMap<String, Question>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecisionResponse {
    pub model: String,
    pub answers: IndexMap<String, Answer>,
    #[serde(default)]
    pub usage: Value,
    /// Fields outside the schema, kept.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl DecisionRequest {
    /// The cache key of one question: sha256 of its id and definition (REQ-SAG-0202).
    pub fn digest(id: &str, question: &Question) -> Result<String, Error> {
        let text =
            serde_json::to_string(&(id, question)).map_err(|e| Error::Encode(e.to_string()))?;
        let hash = sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
            utf8_file_bytes: text.as_bytes(),
        })
        .map_err(|e| Error::Encode(format!("{e:?}")))?;
        Ok(hash.template().to_hex())
    }

    /// Every answer is present and of its question's type; anything else is resent.
    pub fn check(&self, answers: &IndexMap<String, Answer>) -> Result<(), Error> {
        for (id, question) in &self.questions {
            let matches = matches!(
                (question, answers.get(id)),
                (Question::Noul { .. }, Some(Answer::Noul { .. }))
                    | (Question::Choice { .. }, Some(Answer::Choice { .. }))
                    | (Question::Score { .. }, Some(Answer::Score { .. }))
                    | (Question::Box { .. }, Some(Answer::Box { .. }))
            );
            if !matches {
                return Err(Error::Invalid(format!(
                    "answer {id} is missing or not of its question's type"
                )));
            }
        }
        match answers.keys().find(|id| !self.questions.contains_key(*id)) {
            Some(extra) => Err(Error::Invalid(format!("answer {extra} has no question"))),
            None => Ok(()),
        }
    }
}

/// Any decision backend: a [`Codec`] from a [`DecisionRequest`] to a [`DecisionResponse`]
/// (ADR-SAG-0001). The shared client does the sending.
/// Implemented automatically for every [`Codec`] with these request and response types;
/// implement [`Codec`] rather than this trait.
pub trait Backend: Codec<Request = DecisionRequest, Response = DecisionResponse> {}
impl<T: Codec<Request = DecisionRequest, Response = DecisionResponse> + ?Sized> Backend for T {}
