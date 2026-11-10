use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator};
struct RawCapture<'a> {
    name: &'a str,
    start_byte: usize,
    end_byte: usize,
    start: Point,
    end: Point,
}
pub fn extract(input: &ExtractInput<'_>) -> Result<SubjectsDocument, JevError> {
    let path = &input.path;
    let rev = input.rev.clone();
    let blob_sha = &input.blob_sha;
    if input.query.is_empty() {
        return Err(JevError::query_invalid(
            PRODUCER_NAME,
            "query text is empty",
        ));
    }
    let Ok(source) = std::str::from_utf8(input.source) else {
        return Err(JevError::input_invalid("source is not UTF-8"));
    };

    let grammar = grammar::lookup(input.language)?;
    let info = &grammar.info;
    let language = (grammar.load)();
    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|error| {
        JevError::extraction_failed(format!("grammar for {} failed to load: {error}", info.id))
    })?;
    let query = Query::new(&language, input.query)
        .map_err(|error| JevError::query_invalid(PRODUCER_NAME, error.to_string()))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| JevError::extraction_failed("parser returned no tree"))?;

    let capture_names = query.capture_names();
    let mut found = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures() {
            let index = capture.index as usize;
            debug_assert!(
                index < capture_names.len(),
                "tree-sitter capture index is always inside the query"
            );
            let name = capture_names[index];
            if !capture_allowed(name, input.captures) {
                continue;
            }
            found.push(RawCapture {
                name,
                start_byte: capture.node.start_byte(),
                end_byte: capture.node.end_byte(),
                start: capture.node.start_position(),
                end: capture.node.end_position(),
            });
        }
    }
    drop(matches);

    if found.is_empty() {
        return Err(JevError::ExtractionEmpty);
    }

    found.sort_by(|left, right| {
        left.start_byte
            .cmp(&right.start_byte)
            .then_with(|| left.name.cmp(right.name))
            .then_with(|| left.end_byte.cmp(&right.end_byte))
    });

    let root = tree.root_node();
    if root.has_error() {
        let parse_error_line = found
            .iter()
            .filter_map(|capture| error_line_inside(root, capture.start_byte, capture.end_byte))
            .min();
        if let Some(line) = parse_error_line {
            return Err(JevError::extraction_parse_error(
                path.as_str().to_owned(),
                line,
            ));
        }
    }
        let text = source
            .get(capture.start_byte..capture.end_byte)
            .ok_or_else(|| {
                JevError::extraction_invalid(format!(
                    "subject bytes {}..{} are not on UTF-8 character boundaries",
                    capture.start_byte, capture.end_byte
                ))
            })?;
fn capture_allowed(name: &str, captures: Option<&[&str]>) -> bool {
    if name.starts_with('_') {
        return false;
    }
    match captures {
        Some(allow) => allow.contains(&name),
        None => true,
    }
}

fn inclusive_lines(start: Point, end: Point) -> Result<(u32, u32), JevError> {
    let start_line = u32_from_usize("start line", start.row.saturating_add(1))?;
    let end_row = if end.column == 0 && end.row > start.row {
        end.row
    } else {
        end.row.saturating_add(1)
    };
    let end_line = u32_from_usize("end line", end_row.max(start_line as usize))?;
    Ok((start_line, end_line))
}
/// Returns the first line of an `ERROR` or `MISSING` node inside `start..end`.
///
/// A `MISSING` node is zero-width; one at the span end still counts.
fn error_line_inside(root: Node<'_>, start: usize, end: usize) -> Option<u32> {
    let mut found = None;
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
            let line =
                u32::try_from(node.start_position().row.saturating_add(1)).unwrap_or(u32::MAX);
            found = Some(found.map_or(line, |previous: u32| previous.min(line)));
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
    #[cfg(feature = "rust")]
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
        let query = query_source("rust", "match-arms").expect("match-arms");
        let document =
            run("rust", query, source.as_bytes(), "src/classify.rs", None).expect("rust");
        let arms: Vec<&str> = document
            .subjects()
            .iter()
            .map(|subject| subject.text())
            .collect();
        assert_eq!(arms, vec!["Transition::GiveBack", "Transition::Keep"]);
        assert!(
            document
                .subjects()
                .iter()
                .all(|subject| subject.capture() == "arm")
        );
        assert_eq!(document.producer().mode(), PRODUCER_MODE);
        assert_eq!(document.producer().grammar(), "tree-sitter-rust");
        assert_eq!(document.producer().grammar_version(), "0.24.2");
    }
    #[cfg(feature = "python")]
    #[test]
    fn test_tree_sitter_same_line_ids_are_unique_and_stable() {
        let source = b"return Refusal(\"give-back\")\n";
        let query = "(return_statement (call function: (identifier) @arm arguments: (argument_list (string) @refusal)))\n";
        let once = run("python", query, source, "same.py", None).expect("once");
        let twice = run("python", query, source, "same.py", None).expect("twice");
        assert_eq!(once, twice);
        assert_eq!(once.subjects().len(), 2);
        assert_eq!(
            once.subjects()[0].span().start_line(),
            once.subjects()[1].span().start_line()
        );
        assert_eq!(once.subjects()[0].id(), "same.py:1");
        assert_eq!(once.subjects()[1].id(), "same.py:1:1");
        assert!(once.subjects()[0].span().start_byte() < once.subjects()[1].span().start_byte());
        let ids: BTreeSet<&str> = once.subjects().iter().map(|subject| subject.id()).collect();
        assert_eq!(ids.len(), once.subjects().len());
    }

    #[cfg(feature = "python")]
    #[test]
    fn test_tree_sitter_parse_error_is_typed() {
        let error = run("python", "(ERROR) @broken\n", b"def (\n", "broken.py", None)
            .expect_err("syntax error must fail");
        assert_eq!(error.code(), "EXTRACTION_PARSE_ERROR");
        match error {
            JevError::ExtractionParseError { path, line, .. } => {
                assert_eq!(path, "broken.py");
                assert!(line >= 1, "line {line}");
            }
            other => panic!("unexpected {}", other.code()),
        }
    }

    #[test]
    fn test_tree_sitter_unknown_language_is_typed() {
        let error = run("cobol", "(identifier) @name\n", b"x\n", "a.py", None).expect_err("cobol");
        assert_eq!(error.code(), "LANGUAGE_UNAVAILABLE");
        match error {
            JevError::LanguageUnavailable { language, .. } => assert_eq!(language, "cobol"),
            other => panic!("unexpected {}", other.code()),
        }
    }

    #[cfg(feature = "python")]
    #[test]
    fn test_matches_mode_duplicate_captures() {
        let query = "(binary_operator left: (identifier) @name right: (identifier) @name)\n";
        let document =
            run("python", query, b"value = left + right\n", "dup.py", None).expect("dup");
        assert_eq!(document.producer().mode().as_str(), "matches");
        let names: Vec<&str> = document
            .subjects()
            .iter()
            .filter(|subject| subject.capture() == "name")
            .map(|subject| subject.text())
            .collect();
        assert_eq!(names, vec!["left", "right"]);
    }
    #[cfg(feature = "python")]
    #[test]
    fn empty_extraction_is_typed() {
        let error = run(
            "python",
            "(class_definition) @name\n",
            b"pass\n",
            "empty.py",
            None,
        )
        .expect_err("no class");
        assert_eq!(error.code(), "EXTRACTION_EMPTY");
    }

    #[cfg(feature = "python")]
    #[test]
    fn invalid_query_is_typed() {
        let error = run("python", "((( \n", b"pass\n", "bad.py", None).expect_err("query");
        assert_eq!(error.code(), "QUERY_INVALID");
    }
    #[cfg(feature = "python")]
    #[test]
    fn missing_node_inside_subject_is_a_parse_error() {
        // tree-sitter-python recovers `def f(:` with a zero-width MISSING ")"
        // and no ERROR node, so only the is_missing rule catches it.
        let source = b"def f(:\n  pass\n";
        let error = run(
            "python",
            "(function_definition) @def\n",
            source,
            "missing.py",
            None,
        )
        .expect_err("MISSING node must fail");
        assert_eq!(error, JevError::extraction_parse_error("missing.py", 1));
    }

    #[cfg(feature = "python")]
    #[test]
    fn clean_source_skips_the_error_walk_and_succeeds() {
        let document =
            run("python", "(identifier) @name\n", b"a = b\n", "ok.py", None).expect("clean source");
        assert_eq!(document.subjects().len(), 2);
    }
}
