//! The crate's error type.
//!
//! Every variant has a stable id `treesitter.<variant_snake>` ([`Error::id`]) and a one-line
//! message naming the cause and every field ([`Error::message`]). Errors serialize as
//! `{"id": ..., "message": ...}`, the same shape as the workspace's `ErrorDetail`, without
//! depending on any sc-agent crate. Each variant keeps the lower error's kind and message.

use std::fmt;
use std::path::{Path, PathBuf};

/// Every failure sc-treesitter, its PyO3 bindings and `tsq` can report.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The id is not a language this crate knows.
    UnknownLanguage {
        /// The requested language id.
        language: String,
    },
    /// A known language whose grammar feature is not compiled in.
    GrammarUnavailable {
        /// The requested language id.
        language: String,
    },
    /// `tree_sitter::LanguageError` (ABI/version mismatch).
    GrammarLoad {
        /// The language id.
        language: String,
        /// The lower error's `Display`.
        detail: String,
    },
    /// No predefined query by that name for the language.
    UnknownQuery {
        /// The language id.
        language: String,
        /// The requested query name.
        name: String,
    },
    /// `tree_sitter::QueryError`, every field kept.
    QueryCompile {
        /// The language id.
        language: String,
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
        /// `Debug` of `io::ErrorKind` (display only).
        kind: String,
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
        language: String,
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
        language: String,
        /// The query text.
        query: String,
    },
    /// JSON encoding failed.
    Encode {
        /// The `serde_json` error's `Display`.
        detail: String,
    },
    /// A `tsq` argument error.
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
    /// `tsq` could not write stdout or stderr.
    Write {
        /// `Debug` of `io::ErrorKind` (display only).
        kind: String,
        /// The lower error's `Display`.
        detail: String,
    },
    /// `tsq`'s general log could not start.
    Log {
        /// The sc-observability error's code and message.
        detail: String,
    },
}

impl Error {
    /// Returns the stable id `treesitter.<variant_snake>`.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::UnknownLanguage { .. } => "treesitter.unknown_language",
            Self::GrammarUnavailable { .. } => "treesitter.grammar_unavailable",
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
            Self::Log { .. } => "treesitter.log",
        }
    }

    /// Returns one line naming the cause and every field.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownLanguage { language } => format!("unknown language {language:?}"),
            Self::GrammarUnavailable { language } => {
                format!("language {language:?} is known but its grammar feature is not compiled in")
            }
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
            Self::Log { detail } => format!("the general log could not start: {detail}"),
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
        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("id", self.id())?;
        state.serialize_field("message", &self.message())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn one_of_each() -> Vec<Error> {
        let text = || "x".to_owned();
        vec![
            Error::UnknownLanguage { language: text() },
            Error::GrammarUnavailable { language: text() },
            Error::GrammarLoad {
                language: text(),
                detail: text(),
            },
            Error::UnknownQuery {
                language: text(),
                name: text(),
            },
            Error::QueryCompile {
                language: text(),
                offset: 1,
                row: 2,
                column: 3,
                kind: text(),
                detail: text(),
            },
            Error::QueryCaptureInvalid { capture: text() },
            Error::FileNotFound {
                path: text().into(),
            },
            Error::FileRead {
                path: text().into(),
                kind: text(),
                detail: text(),
            },
            Error::NotUtf8 {
                path: None,
                byte_offset: 4,
            },
            Error::InputTooLarge {
                path: None,
                bytes: 5,
                limit: 4,
            },
            Error::ParseFailed { language: text() },
            Error::SyntaxError {
                path: None,
                row: 0,
                column: 1,
                node_kind: text(),
            },
            Error::NoMatches {
                language: text(),
                query: text(),
            },
            Error::Encode { detail: text() },
            Error::Usage {
                kind: text(),
                detail: text(),
            },
            Error::Python { detail: text() },
            Error::Write {
                kind: text(),
                detail: text(),
            },
            Error::Log { detail: text() },
        ]
    }

    /// AC: REQ-SAG-0002 errors.
    #[test]
    fn all_18_ids_are_distinct_well_formed_and_serialized() {
        let errors = one_of_each();
        assert_eq!(errors.len(), 18);
        let ids: BTreeSet<&str> = errors.iter().map(Error::id).collect();
        assert_eq!(ids.len(), 18, "ids must be distinct");
        for error in &errors {
            let id = error.id();
            let rest = id.strip_prefix("treesitter.").expect("prefix");
            // `not_utf8` carries a digit, so the rule is `^treesitter\.[a-z][a-z0-9_]*$`.
            assert!(
                rest.starts_with(|c: char| c.is_ascii_lowercase())
                    && rest
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{id} must match ^treesitter\\.[a-z][a-z0-9_]*$"
            );
            let value = serde_json::to_value(error).expect("serialize");
            assert_eq!(value["id"], id);
            assert_eq!(value["message"], error.message());
            assert_eq!(value.as_object().map(serde_json::Map::len), Some(2));
            assert!(!error.message().contains('\n'), "{id} message is one line");
            assert_eq!(error.to_string(), format!("{id}: {}", error.message()));
        }
    }

    #[test]
    fn multi_line_lower_text_becomes_one_line() {
        let error = Error::Usage {
            kind: "MissingRequiredArgument".to_owned(),
            detail: "error: missing\n\nUsage: tsq query\n".to_owned(),
        };
        assert_eq!(
            error.message(),
            "invalid arguments (MissingRequiredArgument): error: missing Usage: tsq query"
        );
    }
}
