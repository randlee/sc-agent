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
//! `QueryErrorKind` is neither `Clone` nor `Copy`, and clap's `ErrorKind` comes from the
//! optional `cli` dependency, so neither can sit in this always-built, `Clone + Eq` enum.

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::LanguageId;

/// Which resource cap [`Error::LimitExceeded`] hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Limit {
    /// More captures than [`crate::MAX_CAPTURES`].
    Captures,
    /// More total capture text than [`crate::MAX_CAPTURE_TEXT_BYTES`].
    CaptureTextBytes,
    /// More concurrent in-progress matches than [`crate::MATCH_LIMIT`]; tree-sitter would
    /// otherwise drop matches silently.
    InProgressMatches,
}

impl Limit {
    fn as_str(self) -> &'static str {
        match self {
            Self::Captures => "captures",
            Self::CaptureTextBytes => "capture text bytes",
            Self::InProgressMatches => "in-progress matches",
        }
    }
}

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
        /// The I/O error kind (`InvalidInput` when the path is not a regular file).
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
    /// A resource cap was hit while collecting captures.
    LimitExceeded {
        /// The language id.
        language: LanguageId,
        /// Which cap.
        limit: Limit,
        /// The cap's value.
        max: usize,
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
    /// `tsq` could not write stdout.
    ///
    /// A failed stderr write is not reported: there is nowhere left to report it, so `tsq`
    /// drops it and exits 1.
    Write {
        /// The I/O error kind.
        kind: ErrorKind,
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
            Self::LimitExceeded { .. } => "treesitter.limit_exceeded",
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
            Self::LimitExceeded {
                language,
                limit,
                max,
            } => format!(
                "the {language} query exceeded the limit of {max} {}",
                limit.as_str()
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

impl Error {
    /// Returns a static hint saying what the caller does next, when there is one.
    #[must_use]
    pub fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::UnknownLanguage { .. } => {
                Some("run `tsq languages` (or call `languages()`) for the loaded language ids")
            }
            Self::GrammarUnavailable { .. } => {
                Some("build sc-treesitter with the language's `grammar-<language>` feature")
            }
            Self::UnknownQuery { .. } => Some(
                "run `tsq queries --language <language>` (or call `queries()`) for its queries",
            ),
            Self::QueryCompile { .. } => Some("fix the query at the reported row and column"),
            Self::QueryCaptureInvalid { .. } => {
                Some("allow-list only capture names the query defines")
            }
            Self::FileNotFound { .. } => Some("check the file path"),
            Self::NotUtf8 { .. } => Some("pass UTF-8 source text"),
            Self::InputTooLarge { .. } => Some("query a source of at most 8 MiB"),
            Self::SyntaxError { .. } => Some("fix the syntax error inside the matched span"),
            Self::LimitExceeded { .. } => Some("narrow the query or query a smaller source"),
            Self::Usage { .. } => Some("run `tsq --help`"),
            Self::Python { .. } => Some("pass the source as `bytes` or `str`"),
            Self::GrammarLoad { .. }
            | Self::FileRead { .. }
            | Self::ParseFailed { .. }
            | Self::NoMatches { .. }
            | Self::Encode { .. }
            | Self::Write { .. }
            | Self::Log { .. } => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// One value of every variant; needs one compiled-in grammar for the typed language ids.
    #[cfg(any(feature = "grammar-python", feature = "grammar-rust"))]
    fn one_of_each() -> Vec<Error> {
        let text = || "x".to_owned();
        let language = crate::languages().next().expect("a compiled-in grammar");
        vec![
            Error::UnknownLanguage { language: text() },
            Error::GrammarUnavailable { language: text() },
            Error::GrammarLoad {
                language,
                detail: text(),
            },
            Error::UnknownQuery {
                language,
                name: text(),
            },
            Error::QueryCompile {
                language,
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
                kind: ErrorKind::PermissionDenied,
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
            Error::ParseFailed { language },
            Error::SyntaxError {
                path: None,
                row: 0,
                column: 1,
                node_kind: text(),
            },
            Error::NoMatches {
                language,
                query: text(),
            },
            Error::LimitExceeded {
                language,
                limit: Limit::Captures,
                max: 7,
            },
            Error::Encode { detail: text() },
            Error::Usage {
                kind: text(),
                detail: text(),
            },
            Error::Python { detail: text() },
            Error::Write {
                kind: ErrorKind::BrokenPipe,
                detail: text(),
            },
            Error::Log { detail: text() },
        ]
    }

    /// AC: REQ-SAG-0002 errors.
    #[cfg(any(feature = "grammar-python", feature = "grammar-rust"))]
    #[test]
    fn all_19_ids_are_distinct_well_formed_and_serialized() {
        let errors = one_of_each();
        assert_eq!(errors.len(), 19);
        let ids: std::collections::BTreeSet<&str> = errors.iter().map(Error::id).collect();
        assert_eq!(ids.len(), 19, "ids must be distinct");
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
            match error.recovery() {
                Some(recovery) => {
                    assert_eq!(value["recovery"], recovery);
                    assert_eq!(value.as_object().map(serde_json::Map::len), Some(3));
                }
                None => assert_eq!(value.as_object().map(serde_json::Map::len), Some(2)),
            }
            assert!(!error.message().contains('\n'), "{id} message is one line");
            assert_eq!(error.to_string(), format!("{id}: {}", error.message()));
        }
    }

    #[test]
    fn recovery_names_the_next_step() {
        let unknown = Error::UnknownLanguage {
            language: "cobol".to_owned(),
        };
        assert!(
            unknown
                .recovery()
                .is_some_and(|hint| hint.contains("tsq languages"))
        );
        let unavailable = Error::GrammarUnavailable {
            language: "rust".to_owned(),
        };
        assert!(
            unavailable
                .recovery()
                .is_some_and(|hint| hint.contains("grammar-"))
        );
        let write = Error::Write {
            kind: ErrorKind::BrokenPipe,
            detail: "Broken pipe (os error 32)".to_owned(),
        };
        assert_eq!(write.recovery(), None);
        assert_eq!(
            serde_json::to_value(&write).expect("json"),
            serde_json::json!({
                "id": "treesitter.write",
                "message": "cannot write output: broken pipe: Broken pipe (os error 32)"
            })
        );
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
