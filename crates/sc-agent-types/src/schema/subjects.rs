//! `sc/subjects/1` document types.
//!
//! Normative JSON rules live in `docs/sc-agent-types/subjects-schema.md`.
//! [`SubjectsDocument`] is the one validated type: it is built only by
//! [`SubjectsBuilder::build`] or by parsing ([`SubjectsDocument::from_json_slice`]
//! or `Deserialize`), and every path runs the same validation, so a value of
//! this type is always valid.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::JevError;
use crate::ids::{BlobSha, RepoRelativePath, Rev, Sha256Digest};

/// Schema id for the Phase A subjects artifact.
pub const SUBJECTS_SCHEMA_V1: &str = "sc/subjects/1";

/// Git identity of the source the subjects were extracted from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectsSource {
    /// Repo-relative source path.
    pub path: RepoRelativePath,
    /// Git revision, or `HEAD` when the producer recorded that name.
    pub rev: Rev,
    /// Git blob id of the source bytes.
    pub blob_sha: BlobSha,
}

/// Tool that emitted the subjects document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectsProducer {
    /// Producer name, such as `sc-treesitter`.
    pub name: String,
    /// Producer version.
    pub version: String,
    /// Grammar language id.
    pub language: String,
    /// Grammar crate or package name.
    pub grammar: String,
    /// Pinned grammar version.
    pub grammar_version: String,
    /// SHA-256 of the query text that produced these subjects.
    pub query_sha256: Sha256Digest,
    /// Tree-sitter query mode. Phase A producers record `matches`.
    pub mode: String,
}

/// Half-open byte span and inclusive 1-based line span of a subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    start_line: u32,
    end_line: u32,
    start_byte: u32,
    end_byte: u32,
}

impl Span {
    /// Lines are 1-based and inclusive; bytes are 0-based and half-open.
    #[must_use]
    pub const fn new(start_line: u32, end_line: u32, start_byte: u32, end_byte: u32) -> Self {
        Self {
            start_line,
            end_line,
            start_byte,
            end_byte,
        }
    }

    /// First line, 1-based.
    #[must_use]
    pub const fn start_line(&self) -> u32 {
        self.start_line
    }

    /// Last line, 1-based and inclusive.
    #[must_use]
    pub const fn end_line(&self) -> u32 {
        self.end_line
    }

    /// First byte, 0-based.
    #[must_use]
    pub const fn start_byte(&self) -> u32 {
        self.start_byte
    }

    /// Byte one past the end of the subject text.
    #[must_use]
    pub const fn end_byte(&self) -> u32 {
        self.end_byte
    }
}

/// One extracted subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    id: String,
    capture: String,
    text: String,
    span: Span,
}

impl Subject {
    /// A subject with an explicit id.
    ///
    /// Inputs that are not subjects documents (files, table columns) use this
    /// to carry text into the payload. Subjects of a [`SubjectsDocument`] get
    /// their ids from [`SubjectsBuilder`].
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        capture: impl Into<String>,
        text: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            id: id.into(),
            capture: capture.into(),
            text: text.into(),
            span,
        }
    }

    /// `{path}:{start_line}` or `{path}:{start_line}:{n}` for later same-line hits.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Capture name, stored without `@`.
    #[must_use]
    pub fn capture(&self) -> &str {
        &self.capture
    }

    /// UTF-8 slice `[start_byte, end_byte)` of the source.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Location of the text in the source.
    #[must_use]
    pub fn span(&self) -> &Span {
        &self.span
    }
}

/// A validated `sc/subjects/1` document.
///
/// Fields are private. Build one with [`SubjectsBuilder`] or parse one; both
/// run the same validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawSubjectsDocument")]
pub struct SubjectsDocument {
    schema: String,
    source: SubjectsSource,
    producer: SubjectsProducer,
    subjects: Vec<Subject>,
}

/// The wire form, before validation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSubjectsDocument {
    schema: String,
    source: SubjectsSource,
    producer: SubjectsProducer,
    subjects: Vec<Subject>,
}

impl TryFrom<RawSubjectsDocument> for SubjectsDocument {
    type Error = JevError;

    fn try_from(raw: RawSubjectsDocument) -> Result<Self, Self::Error> {
        let document = Self {
            schema: raw.schema,
            source: raw.source,
            producer: raw.producer,
            subjects: raw.subjects,
        };
        document.validate()?;
        Ok(document)
    }
}

impl SubjectsDocument {
    /// Parses and validates a JSON document, keeping the validation error id.
    ///
    /// # Errors
    ///
    /// `EXTRACTION_INVALID` when the bytes are not the schema, and
    /// `EXTRACTION_EMPTY` when `subjects` is empty.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, JevError> {
        let raw: RawSubjectsDocument = serde_json::from_slice(bytes)
            .map_err(|error| JevError::extraction_invalid(format!("subjects JSON: {error}")))?;
        Self::try_from(raw)
    }

    /// Schema id ([`SUBJECTS_SCHEMA_V1`]).
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Source identity.
    #[must_use]
    pub fn source(&self) -> &SubjectsSource {
        &self.source
    }

    /// Producer identity.
    #[must_use]
    pub fn producer(&self) -> &SubjectsProducer {
        &self.producer
    }

    /// Subjects sorted by `(start_byte, capture)`.
    #[must_use]
    pub fn subjects(&self) -> &[Subject] {
        &self.subjects
    }

    /// Checks schema id, id rules, spans, sort order, and the helper-capture
    /// rule. A capture name beginning with `_` must not appear.
    fn validate(&self) -> Result<(), JevError> {
        if self.schema != SUBJECTS_SCHEMA_V1 {
            return Err(JevError::extraction_invalid(format!(
                "schema is {}, expected {SUBJECTS_SCHEMA_V1}",
                self.schema
            )));
        }
        if self.subjects.is_empty() {
            return Err(JevError::ExtractionEmpty);
        }
        if self.producer.mode.is_empty() {
            return Err(JevError::extraction_invalid(
                "subjects source path and producer mode must be non-empty",
            ));
        }
        let path = self.source.path.as_str();
        let mut previous: Option<(u32, &str)> = None;
        let mut per_line: BTreeMap<u32, u32> = BTreeMap::new();
        for subject in &self.subjects {
            if subject.capture.trim_start_matches('@').starts_with('_') {
                return Err(JevError::extraction_invalid(format!(
                    "helper capture {} must not be emitted",
                    subject.capture
                )));
            }
            let span = subject.span;
            if span.start_line == 0 || span.end_line < span.start_line {
                return Err(JevError::extraction_invalid(format!(
                    "span lines are invalid for {}",
                    subject.id
                )));
            }
            if span.end_byte < span.start_byte {
                return Err(JevError::extraction_invalid(format!(
                    "span bytes are invalid for {}",
                    subject.id
                )));
            }
            let width = span.end_byte - span.start_byte;
            if usize::try_from(width).ok() != Some(subject.text.len()) {
                return Err(JevError::extraction_invalid(format!(
                    "text length does not match the span for {}",
                    subject.id
                )));
            }
            let key = (span.start_byte, subject.capture.as_str());
            if previous.is_some_and(|prev| key < prev) {
                return Err(JevError::extraction_invalid(
                    "subjects are not sorted by (start_byte, capture)",
                ));
            }
            previous = Some(key);
            let index = per_line.entry(span.start_line).or_insert(0);
            let expected = subject_id(path, span.start_line, *index);
            *index = index.saturating_add(1);
            if subject.id != expected {
                return Err(JevError::extraction_invalid(format!(
                    "subject id is {}, expected {expected}",
                    subject.id
                )));
            }
        }
        Ok(())
    }
}

/// The one subject-id rule: the first subject on a line is `{path}:{line}`;
/// later ones on that line are `{path}:{line}:1`, `{path}:{line}:2`, ...
fn subject_id(path: &str, line: u32, index_on_line: u32) -> String {
    if index_on_line == 0 {
        format!("{path}:{line}")
    } else {
        format!("{path}:{line}:{index_on_line}")
    }
}

/// Builds a [`SubjectsDocument`], assigning subject ids.
///
/// # Examples
///
/// ```
/// use sc_agent_types::schema::subjects::{Span, SubjectsBuilder, SubjectsProducer, SubjectsSource};
/// use sc_agent_types::{Rev, Sha256Digest};
///
/// let source = SubjectsSource {
///     path: "a.py".parse()?,
///     rev: Rev::new("HEAD"),
///     blob_sha: "b".repeat(40).parse()?,
/// };
/// let producer = SubjectsProducer {
///     name: "sc-treesitter".into(),
///     version: "0.1.0".into(),
///     language: "python".into(),
///     grammar: "tree-sitter-python".into(),
///     grammar_version: "0.23.6".into(),
///     query_sha256: Sha256Digest::of(b"q"),
///     mode: "matches".into(),
/// };
/// let mut builder = SubjectsBuilder::new(source, producer);
/// builder.push_subject("a.py".parse()?, Span::new(1, 1, 4, 5), "b", "name");
/// builder.push_subject("a.py".parse()?, Span::new(1, 1, 0, 1), "a", "name");
/// let document = builder.build()?;
/// assert_eq!(document.subjects()[0].id(), "a.py:1");
/// assert_eq!(document.subjects()[1].id(), "a.py:1:1");
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SubjectsBuilder {
    source: SubjectsSource,
    producer: SubjectsProducer,
    pending: Vec<Pending>,
}

#[derive(Debug, Clone)]
struct Pending {
    path: RepoRelativePath,
    span: Span,
    text: String,
    capture: String,
}

impl SubjectsBuilder {
    /// Starts a document for `source` emitted by `producer`.
    #[must_use]
    pub fn new(source: SubjectsSource, producer: SubjectsProducer) -> Self {
        Self {
            source,
            producer,
            pending: Vec::new(),
        }
    }

    /// Adds one subject; `kind` is the capture name without `@`.
    pub fn push_subject(
        &mut self,
        path: RepoRelativePath,
        span: Span,
        text: impl Into<String>,
        kind: impl Into<String>,
    ) -> &mut Self {
        self.pending.push(Pending {
            path,
            span,
            text: text.into(),
            capture: kind.into(),
        });
        self
    }

    /// Sorts by `(start_byte, capture, end_byte)`, assigns ids, and validates.
    ///
    /// # Errors
    ///
    /// `EXTRACTION_EMPTY` when no subject was pushed, and
    /// `EXTRACTION_INVALID` when a subject breaks a schema rule (for example
    /// a path other than `source.path`, or text that does not match its span).
    pub fn build(self) -> Result<SubjectsDocument, JevError> {
        let mut pending = self.pending;
        pending.sort_by(|left, right| {
            left.span
                .start_byte
                .cmp(&right.span.start_byte)
                .then_with(|| left.capture.cmp(&right.capture))
                .then_with(|| left.span.end_byte.cmp(&right.span.end_byte))
        });
        let mut per_line: BTreeMap<(RepoRelativePath, u32), u32> = BTreeMap::new();
        let mut subjects = Vec::with_capacity(pending.len());
        for item in pending {
            let index = per_line
                .entry((item.path.clone(), item.span.start_line))
                .or_insert(0);
            let id = subject_id(item.path.as_str(), item.span.start_line, *index);
            *index = index.saturating_add(1);
            subjects.push(Subject {
                id,
                capture: item.capture,
                text: item.text,
                span: item.span,
            });
        }
        let document = SubjectsDocument {
            schema: SUBJECTS_SCHEMA_V1.to_owned(),
            source: self.source,
            producer: self.producer,
            subjects,
        };
        document.validate()?;
        Ok(document)
    }
}
