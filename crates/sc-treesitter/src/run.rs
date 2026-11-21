//! Running one query against one source with one grammar.

use std::path::Path;

use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator};

use crate::{Error, LanguageId, MAX_SOURCE_BYTES};

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
/// - [`Error::SyntaxError`] when an `ERROR` or `MISSING` node lies inside a captured span.
pub fn run_query(
    language: LanguageId,
    query: &str,
    source: &[u8],
    path: Option<&Path>,
    captures: Option<&[&str]>,
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

    let mut found = Vec::new();
    let mut cursor = QueryCursor::new();
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
            let (start_byte, end_byte) = (capture.node.start_byte(), capture.node.end_byte());
            found.push(RawCapture {
                name,
                start_byte,
                end_byte,
                start: capture.node.start_position(),
                end: capture.node.end_position(),
            });
        }
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
