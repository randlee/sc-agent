//! The crate's error type.
//!
//! Every variant has a stable id `treesitter.<variant_snake>` ([`Error::id`]) and a one-line
//! message naming the cause and every field ([`Error::message`]), plus an optional static
//! recovery hint ([`Error::recovery`]). Errors serialize as
//! `{"id": ..., "message": ..., "recovery"?: ...}`, the same shape as the workspace's
//! `ErrorDetail` (recovery omitted when there is none), without depending on any sc-agent
//! crate. Each variant keeps the lower error's kind and message.
//!
//! `std::io::ErrorKind` is kept as the typed value. The other `kind` fields
//! ([`Error::QueryCompile`], [`Error::Usage`]) are `Debug` text, for display only: tree-sitter's
//! `QueryErrorKind` is neither `Clone` nor `Copy`, and clap's `ErrorKind` is not stored in this
//! `Clone + Eq` enum.

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::LanguageId;

/// Every failure sc-treesitter, its PyO3 bindings and `sitr` can report.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The id is not a language this crate knows.
    UnknownLanguage {
        /// The requested language id.
        language: String,
    },
    /// `tree_sitter::LanguageError` (ABI/version mismatch).
    GrammarLoad {
        /// The language id.
        language: LanguageId,
        /// The lower error's `Display`.
        detail: String,
    },
    /// No predefined query by that name for the language.
    UnknownQuery {
        /// The language id.
        language: LanguageId,
        /// The requested query name.
        name: String,
    },
    /// `tree_sitter::QueryError`, every field kept.
    QueryCompile {
        /// The language id.
        language: LanguageId,
        /// Byte offset of the error in the query text.
        offset: usize,
        /// 0-based row in the query text.
        row: usize,
        /// 0-based column in the query text.
        column: usize,
        /// `Debug` of `QueryErrorKind` (display only; not a stable value to match on).
        kind: String,
        /// The lower error's message.
        detail: String,
    },
    /// The capture allow-list names a capture the query does not define.
    QueryCaptureInvalid {
        /// The undefined capture name.
        capture: String,
    },
    /// The source file does not exist.
    FileNotFound {
        /// The path that was opened.
        path: PathBuf,
    },
    /// An I/O error other than not-found while reading the source file.
    FileRead {
        /// The path that was read.
        path: PathBuf,
        /// The I/O error kind.
        kind: ErrorKind,
        /// The lower error's `Display`.
        detail: String,
    },
    /// The source is not UTF-8.
    NotUtf8 {
        /// The source path, when known.
        path: Option<PathBuf>,
        /// `Utf8Error::valid_up_to`: the first invalid byte.
        byte_offset: usize,
    },
    /// The source is over [`crate::MAX_SOURCE_BYTES`].
    InputTooLarge {
        /// The source path, when known.
        path: Option<PathBuf>,
        /// The source size in bytes.
        bytes: u64,
        /// The limit in bytes.
        limit: u64,
    },
    /// `Parser::parse` returned no tree.
    ParseFailed {
        /// The language id.
        language: LanguageId,
    },
    /// An `ERROR` or `MISSING` node inside a matched span.
    SyntaxError {
        /// The source path, when known.
        path: Option<PathBuf>,
        /// 0-based row of the earliest such node.
        row: usize,
        /// 0-based column of the earliest such node.
        column: usize,
        /// The node kind (`ERROR`, or `MISSING <kind>`).
        node_kind: String,
    },
    /// The query ran and matched nothing (after helper and allow-list filtering).
    NoMatches {
        /// The language id.
        language: LanguageId,
        /// The query text.
        query: String,
    },
    /// JSON encoding failed.
    Encode {
        /// The `serde_json` error's `Display`.
        detail: String,
    },
    /// A `sitr` argument error.
    Usage {
        /// `Debug` of clap's `ErrorKind` (display only).
        kind: String,
        /// clap's rendered text.
        detail: String,
    },
    /// A PyO3 argument conversion failure (bindings only).
    Python {
        /// The `PyErr` `Display`.
        detail: String,
    },
    /// `sitr` could not write stdout.
    ///
    /// A failed stderr write is not reported: there is nowhere left to report it, so `sitr`
    /// drops it and exits 1.
    Write {
        /// The I/O error kind.
        kind: ErrorKind,
        /// The lower error's `Display`.
        detail: String,
    },
}

impl Error {
    /// Returns the stable id `treesitter.<variant_snake>`.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::UnknownLanguage { .. } => "treesitter.unknown_language",
            Self::GrammarLoad { .. } => "treesitter.grammar_load",
            Self::UnknownQuery { .. } => "treesitter.unknown_query",
            Self::QueryCompile { .. } => "treesitter.query_compile",
            Self::QueryCaptureInvalid { .. } => "treesitter.query_capture_invalid",
            Self::FileNotFound { .. } => "treesitter.file_not_found",
            Self::FileRead { .. } => "treesitter.file_read",
            Self::NotUtf8 { .. } => "treesitter.not_utf8",
            Self::InputTooLarge { .. } => "treesitter.input_too_large",
            Self::ParseFailed { .. } => "treesitter.parse_failed",
            Self::SyntaxError { .. } => "treesitter.syntax_error",
            Self::NoMatches { .. } => "treesitter.no_matches",
            Self::Encode { .. } => "treesitter.encode",
            Self::Usage { .. } => "treesitter.usage",
            Self::Python { .. } => "treesitter.python",
            Self::Write { .. } => "treesitter.write",
        }
    }

    /// Returns one line naming the cause and every field.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownLanguage { language } => format!("unknown language {language:?}"),
            Self::GrammarLoad { language, detail } => {
                format!("grammar for {language:?} failed to load: {detail}")
            }
            Self::UnknownQuery { language, name } => {
                format!("no predefined query {name:?} for language {language:?}")
            }
            Self::QueryCompile {
                language,
                offset,
                row,
                column,
                kind,
                detail,
            } => format!(
                "{language} query does not compile at row {row}, column {column} \
                 (offset {offset}): {kind}: {}",
                one_line(detail)
            ),
            Self::QueryCaptureInvalid { capture } => {
                format!("capture {capture:?} is not defined by the query")
            }
            Self::FileNotFound { path } => format!("file {} not found", path.display()),
            Self::FileRead { path, kind, detail } => {
                format!("cannot read {}: {kind}: {detail}", path.display())
            }
            Self::NotUtf8 { path, byte_offset } => format!(
                "source{} is not UTF-8 at byte {byte_offset}",
                in_path(path.as_deref())
            ),
            Self::InputTooLarge { path, bytes, limit } => format!(
                "source{} is {bytes} bytes, over the {limit}-byte limit",
                in_path(path.as_deref())
            ),
            Self::ParseFailed { language } => {
                format!("the {language} parser returned no tree")
            }
            Self::SyntaxError {
                path,
                row,
                column,
                node_kind,
            } => format!(
                "syntax error{}: {node_kind} node at row {row}, column {column} (0-based) \
                 inside a matched span",
                in_path(path.as_deref())
            ),
            Self::NoMatches { language, query } => format!(
                "query {:?} matched nothing in {language} source",
                query.trim()
            ),
            Self::Encode { detail } => format!("JSON encoding failed: {detail}"),
            Self::Usage { kind, detail } => {
                format!("invalid arguments ({kind}): {}", one_line(detail))
            }
            Self::Python { detail } => format!("invalid Python argument: {detail}"),
            Self::Write { kind, detail } => format!("cannot write output: {kind}: {detail}"),
        }
    }
}

impl Error {
    /// Returns a static hint saying what the caller does next, when there is one.
    #[must_use]
    pub fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::UnknownLanguage { .. } => {
                Some("run `sitr languages` (or call `languages()`) for the loaded language ids")
            }
            Self::UnknownQuery { .. } => Some(
                "run `sitr queries --language <language>` (or call `queries()`) for its queries",
            ),
            Self::QueryCompile { .. } => Some("fix the query at the reported row and column"),
            Self::QueryCaptureInvalid { .. } => {
                Some("allow-list only capture names the query defines")
            }
            Self::FileNotFound { .. } => Some("check the file path"),
            Self::NotUtf8 { .. } => Some("pass UTF-8 source text"),
            Self::InputTooLarge { .. } => Some("query a source of at most 8 MiB"),
            Self::SyntaxError { .. } => Some("fix the syntax error inside the matched span"),
            Self::Usage { .. } => Some("run `sitr --help`"),
            Self::Python { .. } => Some("pass the source as `bytes` or `str`"),
            Self::GrammarLoad { .. }
            | Self::FileRead { .. }
            | Self::ParseFailed { .. }
            | Self::NoMatches { .. }
            | Self::Encode { .. }
            | Self::Write { .. } => None,
        }
    }
}

/// `" in <path>"`, or nothing when the path is unknown.
fn in_path(path: Option<&Path>) -> String {
    path.map_or_else(String::new, |path| format!(" in {}", path.display()))
}

/// Joins multi-line lower-error text into one line.
fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.id(), self.message())
    }
}

impl std::error::Error for Error {}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let recovery = self.recovery();
        let fields = if recovery.is_some() { 3 } else { 2 };
        let mut state = serializer.serialize_struct("Error", fields)?;
        state.serialize_field("id", self.id())?;
        state.serialize_field("message", &self.message())?;
        if let Some(recovery) = recovery {
            state.serialize_field("recovery", recovery)?;
        } else {
            state.skip_field("recovery")?;
        }
        state.end()
    }
}
