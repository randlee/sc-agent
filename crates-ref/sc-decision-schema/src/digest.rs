//! [`QuestionDigest`]: the per-question sc-sha digest of everything one send carries (REQ-SAG-0202).
//!
//! For each question id `q`, in request order, the digest input is the compact JSON
//! (serde_json with `preserve_order`) of
//!
//! ```json
//! {"body": {"model": "...", "state": "...", "questions": {"q": "<question q>"}, "assets": "..."},
//!  "long_context": false,
//!  "parts": [{"name": "...", "format": "...", "sha256": "<sc-sha hex of the lowercase hex of the bytes>"}]}
//! ```
//!
//! hashed by sc-sha. Model, state, assets, every image part and the long-context header are
//! covered; of the questions only `q` is, so changing another question leaves `q`'s digest
//! unchanged. The images header is derived from the body, so it is covered too. sc-sha hashes
//! UTF-8 text only, so each part's bytes are hex-encoded and hashed once per part. Only sc-sha
//! computes SHA-256 in this crate.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::DecisionError;
use crate::decision::ImagePart;
use crate::map::OrderedMap;
use crate::request::{Asset, ImageFormat, Question, State, SystemOneRequest};

/// The SHA-256 of one question and everything sent with it, as 64 lowercase hex chars.
///
/// Serialised as a JSON string, the form ADR-SAG-0002 session rows carry. Deserialize
/// validates: any string other than 64 lowercase hex chars is refused.
///
/// # Examples
///
/// ```
/// use sc_decision_schema::{DecisionRequest, SystemOneRequest};
///
/// let body = SystemOneRequest::from_json(
///     r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#,
/// )?;
/// let request = DecisionRequest::try_from(body)?;
/// let (id, digest) = request.digests().iter().next().expect("one question");
/// assert_eq!(id, "q");
/// assert_eq!(digest.as_str().len(), 64);
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct QuestionDigest(String);

impl QuestionDigest {
    /// The sc-sha digest of `text`.
    ///
    /// Compact serde_json escapes CR inside strings, so sc-sha's CRLF normalisation never
    /// alters the digest input built by [`question_digests`].
    pub(crate) fn of_text(text: &str) -> Result<Self, sc_sha::ShaError> {
        let input = sc_sha::HashInput::TextFileBytes {
            utf8_file_bytes: text.as_bytes(),
        };
        match sc_sha::calculate_hash(input) {
            Ok(result) => Ok(Self(result.template().to_hex())),
            Err(error) => Err(error),
        }
    }

    /// The digest as 64 lowercase hex chars.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for QuestionDigest {
    type Error = sc_sha::ShaError;

    /// Accepts exactly 64 lowercase hex chars, else [`sc_sha::ShaError::InvalidDigestHex`].
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let lowercase_hex = value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if value.len() == 64 && lowercase_hex {
            Ok(Self(value))
        } else {
            Err(sc_sha::ShaError::InvalidDigestHex)
        }
    }
}

impl From<QuestionDigest> for String {
    fn from(digest: QuestionDigest) -> Self {
        digest.0
    }
}

impl AsRef<str> for QuestionDigest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QuestionDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The digest input of one question (see the module docs).
#[derive(Serialize)]
struct DigestInput<'a> {
    body: DigestBody<'a>,
    long_context: bool,
    parts: &'a [PartDigest<'a>],
}

/// The request body with `questions` narrowed to the one question.
#[derive(Serialize)]
struct DigestBody<'a> {
    model: &'a str,
    state: &'a State,
    questions: BTreeMap<&'a str, &'a Question>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assets: Option<&'a OrderedMap<String, Asset>>,
}

/// One image part as it enters the digest: its bytes replaced by their sc-sha digest.
#[derive(Serialize)]
struct PartDigest<'a> {
    name: &'a str,
    format: ImageFormat,
    sha256: String,
}

/// One digest per question of `body`, in request order.
///
/// # Errors
///
/// [`DecisionError::Encode`] when serde_json refuses the digest input, or (unreachable by
/// sc-sha 1.8's contract: a `&str` is valid UTF-8) when sc-sha refuses it.
pub(crate) fn question_digests(
    body: &SystemOneRequest,
    image_parts: &[ImagePart],
    long_context: bool,
) -> Result<OrderedMap<String, QuestionDigest>, DecisionError> {
    let parts = image_parts
        .iter()
        .map(|part| {
            Ok(PartDigest {
                name: &part.name,
                format: part.format,
                sha256: digest_text(&lowercase_hex(&part.bytes))?.into(),
            })
        })
        .collect::<Result<Vec<_>, DecisionError>>()?;
    let digests = body
        .questions
        .iter()
        .map(|(id, question)| {
            let input = DigestInput {
                body: DigestBody {
                    model: &body.model,
                    state: &body.state,
                    questions: BTreeMap::from([(id.as_str(), question)]),
                    assets: body.assets.as_ref(),
                },
                long_context,
                parts: &parts,
            };
            let text = serde_json::to_string(&input).map_err(|error| DecisionError::Encode {
                message: format!("the digest input of question `{id}` does not serialise: {error}"),
            })?;
            Ok((id.clone(), digest_text(&text)?))
        })
        .collect::<Result<Vec<_>, DecisionError>>()?;
    // The question ids are already distinct: they come from an `OrderedMap`.
    OrderedMap::try_from_pairs(digests)
}

fn digest_text(text: &str) -> Result<QuestionDigest, DecisionError> {
    QuestionDigest::of_text(text).map_err(|error| DecisionError::Encode {
        message: format!("sc-sha refused the digest input: {error}"),
    })
}

/// The lowercase hex digit of the low four bits of `nibble`.
fn hex_digit(nibble: u8) -> char {
    let value = nibble & 0x0f;
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        hex.push(hex_digit(byte >> 4));
        hex.push(hex_digit(*byte));
    }
    hex
}
