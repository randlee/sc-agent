//! Running one query against one source with one grammar.

use std::path::Path;

use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator};

use crate::{Error, LanguageId, Limit, MAX_SOURCE_BYTES};

/// The most captures one [`run_query`] call returns.
pub const MAX_CAPTURES: usize = 100_000;

/// The most capture text, summed over all captures, one [`run_query`] call returns: 64 MiB.
///
/// Nested captures repeat source text, so the total can exceed [`MAX_SOURCE_BYTES`].
pub const MAX_CAPTURE_TEXT_BYTES: usize = 64 * 1024 * 1024;

/// The most concurrent in-progress matches tree-sitter may track (its cap is 65 536).
///
/// Past it tree-sitter drops matches silently; [`run_query`] reports that instead.
pub const MATCH_LIMIT: u32 = 4096;

/// The caps one run enforces; tests use smaller ones.
#[derive(Debug, Clone, Copy)]
struct Limits {
    captures: usize,
    text_bytes: usize,
    matches: u32,
}

const LIMITS: Limits = Limits {
    captures: MAX_CAPTURES,
    text_bytes: MAX_CAPTURE_TEXT_BYTES,
    matches: MATCH_LIMIT,
};

/// One captured node of one query match.
///
/// Lines are 1-based and inclusive; bytes are 0-based, `end_byte` exclusive.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, get_all, skip_from_py_object, module = "sc_treesitter")
)]
#[non_exhaustive]
pub struct Capture {
    /// The capture name, without the `@`.
    pub name: String,
    /// The captured source text.
    pub text: String,
    /// 1-based first line.
    pub start_line: usize,
    /// 1-based last line, inclusive.
    pub end_line: usize,
    /// 0-based first byte.
    pub start_byte: usize,
    /// 0-based end byte, exclusive.
    pub end_byte: usize,
}

struct RawCapture<'a> {
    name: &'a str,
    start_byte: usize,
    end_byte: usize,
    start: Point,
    end: Point,
}

/// Runs any query against any loaded grammar; captures sorted by (start_byte, name, end_byte).
///
/// Names starting with `_` are helpers and never returned; `captures` is an allow-list of
/// capture names to return. `path` is used only in error details.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "grammar-rust")] {
/// let rust = sc_treesitter::LanguageId::new("rust")?;
/// let captures = sc_treesitter::run_query(
///     rust,
///     "(function_item name: (identifier) @name)",
///     b"fn main() {}\n",
///     None,
///     None,
/// )?;
/// assert_eq!(captures[0].text, "main");
/// assert_eq!(captures[0].start_line, 1);
/// # }
/// # Ok::<(), sc_treesitter::Error>(())
/// ```
///
/// # Errors
///
/// - [`Error::InputTooLarge`] when `source` is over [`MAX_SOURCE_BYTES`];
/// - [`Error::NotUtf8`] when `source` is not UTF-8;
/// - [`Error::GrammarLoad`] when tree-sitter rejects the grammar;
/// - [`Error::QueryCompile`] when the query does not compile;
/// - [`Error::QueryCaptureInvalid`] when `captures` names a capture the query lacks;
/// - [`Error::ParseFailed`] when the parser returns no tree;
/// - [`Error::NoMatches`] when nothing is captured;
/// - [`Error::LimitExceeded`] past [`MAX_CAPTURES`], [`MAX_CAPTURE_TEXT_BYTES`] or
///   [`MATCH_LIMIT`];
/// - [`Error::SyntaxError`] when an `ERROR` or `MISSING` node lies inside a captured span.
pub fn run_query(
    language: LanguageId,
    query: &str,
    source: &[u8],
    path: Option<&Path>,
    captures: Option<&[&str]>,
) -> Result<Vec<Capture>, Error> {
    run_with_limits(language, query, source, path, captures, LIMITS)
}

fn run_with_limits(
    language: LanguageId,
    query: &str,
    source: &[u8],
    path: Option<&Path>,
    captures: Option<&[&str]>,
    limits: Limits,
) -> Result<Vec<Capture>, Error> {
    let owned_path = || path.map(Path::to_path_buf);
    if source.len() > MAX_SOURCE_BYTES {
        return Err(Error::InputTooLarge {
            path: owned_path(),
            bytes: source.len() as u64,
            limit: MAX_SOURCE_BYTES as u64,
        });
    }
    let source = std::str::from_utf8(source).map_err(|error| Error::NotUtf8 {
        path: owned_path(),
        byte_offset: error.valid_up_to(),
    })?;

    let tree_language = (language.grammar().load)();
    let mut parser = Parser::new();
    parser
        .set_language(&tree_language)
        .map_err(|error| Error::GrammarLoad {
            language,
            detail: error.to_string(),
        })?;
    let compiled = Query::new(&tree_language, query).map_err(|error| Error::QueryCompile {
        language,
        offset: error.offset,
        row: error.row,
        column: error.column,
        kind: format!("{:?}", error.kind),
        detail: error.message,
    })?;
    let capture_names = compiled.capture_names();
    if let Some(allow) = captures
        && let Some(undefined) = allow.iter().find(|name| !capture_names.contains(name))
    {
        return Err(Error::QueryCaptureInvalid {
            capture: (*undefined).to_owned(),
        });
    }
    let tree = parser
        .parse(source, None)
        .ok_or(Error::ParseFailed { language })?;

    let limit_exceeded = |limit, max| Error::LimitExceeded {
        language,
        limit,
        max,
    };
    let mut found = Vec::new();
    let mut text_bytes = 0usize;
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(limits.matches);
    let mut matches = cursor.matches(&compiled, tree.root_node(), source.as_bytes());
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures() {
            // tree-sitter guarantees the index is inside the query; skip it otherwise.
            let Some(&name) = capture_names.get(capture.index as usize) else {
                continue;
            };
            if !capture_allowed(name, captures) {
                continue;
            }
            if found.len() >= limits.captures {
                return Err(limit_exceeded(Limit::Captures, limits.captures));
            }
            let (start_byte, end_byte) = (capture.node.start_byte(), capture.node.end_byte());
            text_bytes = text_bytes.saturating_add(end_byte.saturating_sub(start_byte));
            if text_bytes > limits.text_bytes {
                return Err(limit_exceeded(Limit::CaptureTextBytes, limits.text_bytes));
            }
            found.push(RawCapture {
                name,
                start_byte,
                end_byte,
                start: capture.node.start_position(),
                end: capture.node.end_position(),
            });
        }
    }
    drop(matches);
    if cursor.did_exceed_match_limit() {
        return Err(limit_exceeded(
            Limit::InProgressMatches,
            limits.matches as usize,
        ));
    }

    if found.is_empty() {
        return Err(Error::NoMatches {
            language,
            query: query.to_owned(),
        });
    }

    found.sort_by(|left, right| {
        left.start_byte
            .cmp(&right.start_byte)
            .then_with(|| left.name.cmp(right.name))
            .then_with(|| left.end_byte.cmp(&right.end_byte))
    });

    let root = tree.root_node();
    if root.has_error() {
        let earliest = found
            .iter()
            .filter_map(|capture| error_inside(root, capture.start_byte, capture.end_byte))
            .min();
        if let Some((row, column, node_kind)) = earliest {
            return Err(Error::SyntaxError {
                path: owned_path(),
                row,
                column,
                node_kind,
            });
        }
    }

    let mut out = Vec::with_capacity(found.len());
    for capture in found {
        // Node boundaries in validated UTF-8 are character boundaries; skip one otherwise.
        let Some(text) = source.get(capture.start_byte..capture.end_byte) else {
            continue;
        };
        let (start_line, end_line) = inclusive_lines(capture.start, capture.end);
        out.push(Capture {
            name: capture.name.to_owned(),
            text: text.to_owned(),
            start_line,
            end_line,
            start_byte: capture.start_byte,
            end_byte: capture.end_byte,
        });
    }
    Ok(out)
}

fn capture_allowed(name: &str, captures: Option<&[&str]>) -> bool {
    if name.starts_with('_') {
        return false;
    }
    match captures {
        Some(allow) => allow.contains(&name),
        None => true,
    }
}

/// 1-based inclusive lines of a node spanning `start..end`.
///
/// A node ending at column 0 of a later row ends on the previous line. Rows are below 8 Mi
/// (the source is at most [`MAX_SOURCE_BYTES`]), so `saturating_add` never saturates.
fn inclusive_lines(start: Point, end: Point) -> (usize, usize) {
    let start_line = start.row.saturating_add(1);
    let end_row = if end.column == 0 && end.row > start.row {
        end.row
    } else {
        end.row.saturating_add(1)
    };
    (start_line, end_row.max(start_line))
}

/// Returns `(row, column, kind)` of the earliest `ERROR` or `MISSING` node inside `start..end`.
///
/// A `MISSING` node is zero-width; one at the span end still counts.
fn error_inside(root: Node<'_>, start: usize, end: usize) -> Option<(usize, usize, String)> {
    let mut found: Option<(usize, usize, String)> = None;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let node_start = node.start_byte();
        let node_end = node.end_byte();
        if node_end < start || node_start > end || !node.has_error() {
            continue;
        }
        let inside = if node.is_missing() {
            node_start >= start && node_end <= end
        } else {
            node.is_error() && node_start >= start && node_start < end && node_end <= end
        };
        if inside {
            let position = node.start_position();
            let kind = if node.is_missing() {
                format!("MISSING {}", node.kind())
            } else {
                node.kind().to_owned()
            };
            let candidate = (position.row, position.column, kind);
            if found.as_ref().is_none_or(|previous| candidate < *previous) {
                found = Some(candidate);
            }
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "grammar-rust")]
    use crate::query_source;

    #[cfg(any(feature = "grammar-python", feature = "grammar-rust"))]
    fn lang(id: &str) -> LanguageId {
        LanguageId::new(id).expect("compiled-in language")
    }

    #[cfg(any(feature = "grammar-python", feature = "grammar-rust"))]
    fn texts(captures: &[Capture]) -> Vec<&str> {
        captures
            .iter()
            .map(|capture| capture.text.as_str())
            .collect()
    }

    #[cfg(feature = "grammar-rust")]
    #[test]
    fn test_tree_sitter_rust_match_arms() {
        let source = r#"
fn classify(value: Transition) -> &'static str {
    match value {
        Transition::GiveBack => "give-back",
        Transition::Keep => "keep",
    }
}
"#;
        let query = query_source(lang("rust"), "match-arms").expect("match-arms");
        let captures = run_query(lang("rust"), query, source.as_bytes(), None, None).expect("rust");
        assert_eq!(
            texts(&captures),
            vec!["Transition::GiveBack", "Transition::Keep"]
        );
        assert!(captures.iter().all(|capture| capture.name == "arm"));
        assert_eq!(captures[0].start_line, 4);
        assert_eq!(captures[1].start_line, 5);
    }

    #[cfg(feature = "grammar-python")]
    #[test]
    fn test_tree_sitter_same_line_captures_are_stable_and_byte_ordered() {
        let source = b"return Refusal(\"give-back\")\n";
        let query = "(return_statement (call function: (identifier) @arm arguments: (argument_list (string) @refusal)))\n";
        let once = run_query(lang("python"), query, source, None, None).expect("once");
        let twice = run_query(lang("python"), query, source, None, None).expect("twice");
        assert_eq!(once, twice);
        assert_eq!(once.len(), 2);
        assert_eq!(once[0].start_line, once[1].start_line);
        assert!(once[0].start_byte < once[1].start_byte);
        assert_eq!(texts(&once), ["Refusal", "\"give-back\""]);
    }

    /// AC: REQ-SAG-0002 errors.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn test_tree_sitter_parse_error_is_typed() {
        let path = Path::new("broken.py");
        let error = run_query(
            lang("python"),
            "(ERROR) @broken\n",
            b"def (\n",
            Some(path),
            None,
        )
        .expect_err("syntax error must fail");
        assert_eq!(error.id(), "treesitter.syntax_error");
        match error {
            Error::SyntaxError {
                path: Some(found),
                row,
                node_kind,
                ..
            } => {
                assert_eq!(found, path);
                assert_eq!(row, 0);
                assert_eq!(node_kind, "ERROR");
            }
            other => panic!("unexpected {other}"),
        }
    }

    /// AC: REQ-SAG-0002 errors.
    #[test]
    fn test_tree_sitter_unknown_language_is_typed() {
        let error = LanguageId::new("cobol").expect_err("cobol");
        assert_eq!(error.id(), "treesitter.unknown_language");
        assert!(matches!(error, Error::UnknownLanguage { language } if language == "cobol"));
    }

    #[cfg(feature = "grammar-python")]
    #[test]
    fn test_matches_mode_duplicate_captures() {
        let query = "(binary_operator left: (identifier) @name right: (identifier) @name)\n";
        let captures =
            run_query(lang("python"), query, b"value = left + right\n", None, None).expect("dup");
        let names: Vec<&str> = captures
            .iter()
            .filter(|capture| capture.name == "name")
            .map(|capture| capture.text.as_str())
            .collect();
        assert_eq!(names, vec!["left", "right"]);
    }

    /// AC: REQ-SAG-0002 errors.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn empty_result_is_no_matches() {
        let query = "(class_definition) @name\n";
        let error = run_query(lang("python"), query, b"pass\n", None, None).expect_err("no class");
        assert_eq!(error.id(), "treesitter.no_matches");
        assert_eq!(
            error,
            Error::NoMatches {
                language: lang("python"),
                query: query.to_owned()
            }
        );
        // An empty query compiles to zero patterns and matches nothing.
        let empty = run_query(lang("python"), "", b"pass\n", None, None).expect_err("empty");
        assert!(matches!(empty, Error::NoMatches { .. }));
    }

    /// AC: REQ-SAG-0002 errors.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn invalid_query_is_query_compile() {
        let error = run_query(
            lang("python"),
            "(identifier) @a\n(nope) @b\n",
            b"pass\n",
            None,
            None,
        )
        .expect_err("query");
        assert_eq!(error.id(), "treesitter.query_compile");
        match error {
            Error::QueryCompile {
                language,
                offset,
                row,
                column,
                kind,
                detail,
            } => {
                assert_eq!(language.as_str(), "python");
                assert_eq!((row, column), (1, 1));
                assert_eq!(offset, 17);
                assert_eq!(kind, "NodeType");
                assert!(detail.contains("nope"), "{detail}");
            }
            other => panic!("unexpected {other}"),
        }
    }

    /// AC: REQ-SAG-0002 errors.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn missing_node_inside_capture_is_a_syntax_error() {
        // tree-sitter-python recovers `def f(:` with a zero-width MISSING ")"
        // and no ERROR node, so only the is_missing rule catches it.
        let source = b"def f(:\n  pass\n";
        let error = run_query(
            lang("python"),
            "(function_definition) @def\n",
            source,
            None,
            None,
        )
        .expect_err("MISSING node must fail");
        assert_eq!(
            error,
            Error::SyntaxError {
                path: None,
                row: 0,
                column: 6,
                node_kind: "MISSING )".to_owned(),
            }
        );
    }

    #[cfg(feature = "grammar-python")]
    #[test]
    fn clean_source_skips_the_error_walk_and_succeeds() {
        let captures = run_query(
            lang("python"),
            "(identifier) @name\n",
            b"a = b\n",
            None,
            None,
        )
        .expect("clean");
        assert_eq!(texts(&captures), ["a", "b"]);
    }

    /// AC: REQ-SAG-0302 same call.
    #[cfg(all(feature = "grammar-python", feature = "grammar-rust"))]
    #[test]
    fn python_and_rust_through_the_same_call() {
        // One caller-written query shape, one call, two grammars.
        let calls = [
            (
                "python",
                "(function_definition name: (identifier) @name)",
                "def hello():\n    pass\n",
            ),
            (
                "rust",
                "(function_item name: (identifier) @name)",
                "fn hello() {}\n",
            ),
        ];
        for (language, query, source) in calls {
            let captures = run_query(lang(language), query, source.as_bytes(), None, None)
                .unwrap_or_else(|error| panic!("{language}: {error}"));
            assert_eq!(texts(&captures), ["hello"], "{language}");
            assert_eq!(captures[0].start_line, 1);
        }
    }

    /// Per-language sample: (id, query, source, expected first capture text).
    /// A new grammar adds a row here.
    const SAMPLES: &[(&str, &str, &str, &str)] = &[
        ("python", "(identifier) @id", "answer = 42\n", "answer"),
        ("rust", "(identifier) @id", "fn answer() {}\n", "answer"),
    ];

    /// AC: REQ-SAG-0302 every id.
    #[test]
    fn every_loaded_language_runs_through_run_query() {
        let mut ran = 0;
        for id in crate::languages() {
            let (_, query, source, expected) = SAMPLES
                .iter()
                .find(|sample| sample.0 == id.as_str())
                .unwrap_or_else(|| panic!("no sample for loaded language {id}"));
            let captures = run_query(id, query, source.as_bytes(), None, None)
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            assert_eq!(captures[0].text, *expected, "{id}");
            ran += 1;
        }
        assert_eq!(ran, crate::languages().len());
    }

    /// AC: REQ-SAG-0002 errors.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn helper_captures_are_hidden_and_allow_list_is_checked() {
        let query = "(assignment left: (identifier) @_lhs right: (integer) @value)\n";
        let captures = run_query(lang("python"), query, b"x = 1\n", None, None).expect("run");
        assert_eq!(texts(&captures), ["1"]);
        assert!(captures.iter().all(|capture| capture.name == "value"));

        let allowed =
            run_query(lang("python"), query, b"x = 1\n", None, Some(&["value"])).expect("ok");
        assert_eq!(allowed, captures);

        let error = run_query(
            lang("python"),
            query,
            b"x = 1\n",
            None,
            Some(&["value", "nope"]),
        )
        .expect_err("undefined capture");
        assert_eq!(error.id(), "treesitter.query_capture_invalid");
        assert_eq!(
            error,
            Error::QueryCaptureInvalid {
                capture: "nope".to_owned()
            }
        );
    }

    /// AC: Positions, REQ-SAG-0002 errors.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn not_utf8_too_large_and_last_line_positions() {
        let path = Path::new("bad.py");
        let error = run_query(
            lang("python"),
            "(identifier) @id",
            b"ab = 1\n\xff\n",
            Some(path),
            None,
        )
        .expect_err("not utf8");
        assert_eq!(
            error,
            Error::NotUtf8 {
                path: Some(path.to_path_buf()),
                byte_offset: 7
            }
        );

        let big = vec![b' '; MAX_SOURCE_BYTES + 1];
        let error =
            run_query(lang("python"), "(identifier) @id", &big, None, None).expect_err("big");
        assert_eq!(
            error,
            Error::InputTooLarge {
                path: None,
                bytes: (MAX_SOURCE_BYTES + 1) as u64,
                limit: MAX_SOURCE_BYTES as u64
            }
        );

        let source = b"a = 1\nb = 2\nlast = [\n  3,\n]";
        let captures =
            run_query(lang("python"), "(list) @list", source, None, None).expect("multi-line");
        assert_eq!(captures.len(), 1);
        assert_eq!((captures[0].start_line, captures[0].end_line), (3, 5));
        assert_eq!(captures[0].end_byte, source.len());
    }

    /// RSH-002: every cap is a typed error, never a silent truncation.
    #[cfg(feature = "grammar-python")]
    #[test]
    fn capture_text_and_match_limits_are_typed() {
        let python = lang("python");
        let source = b"a = b + c + d + e\n";
        let query = "(identifier) @id";
        let small = |captures, text_bytes, matches| Limits {
            captures,
            text_bytes,
            matches,
        };
        let ok = run_with_limits(python, query, source, None, None, small(5, 5, 8)).expect("5");
        assert_eq!(texts(&ok), ["a", "b", "c", "d", "e"]);

        let error = run_with_limits(python, query, source, None, None, small(4, 100, 8))
            .expect_err("captures");
        assert_eq!(
            error,
            Error::LimitExceeded {
                language: python,
                limit: Limit::Captures,
                max: 4
            }
        );
        assert_eq!(error.id(), "treesitter.limit_exceeded");

        let error = run_with_limits(python, query, source, None, None, small(100, 4, 8))
            .expect_err("text bytes");
        assert!(matches!(
            error,
            Error::LimitExceeded {
                limit: Limit::CaptureTextBytes,
                max: 4,
                ..
            }
        ));

        // Nested binary operators keep several matches in progress at once.
        let nested = b"x = a + b + c + d + e + f + g + h\n";
        let pairs = "(binary_operator left: (_) @l right: (_) @r)";
        assert!(run_with_limits(python, pairs, nested, None, None, LIMITS).is_ok());
        let error = run_with_limits(python, pairs, nested, None, None, small(100, 1000, 1))
            .expect_err("match limit");
        assert!(matches!(
            error,
            Error::LimitExceeded {
                limit: Limit::InProgressMatches,
                max: 1,
                ..
            }
        ));
    }
}
