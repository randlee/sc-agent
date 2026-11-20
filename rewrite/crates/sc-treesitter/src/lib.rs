use serde_json::{Value, json};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

pub fn languages() -> Value {
    json!({"ok": true, "value": ["python", "rust", "csharp", "go"]})
}

pub fn run_query(language: &str, source_query: &str, source: &str) -> Value {
    let language = match language {
        "python" => tree_sitter_python::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "csharp" => tree_sitter_c_sharp::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        _ => {
            return sc_agent::error(
                "unsupported_language",
                "Language is not supported",
                "Choose a supported language",
            );
        }
    };
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return sc_agent::error(
            "invalid_language",
            "Cannot load grammar",
            "Check installation",
        );
    }
    let Some(tree) = parser.parse(source, None) else {
        return sc_agent::error("parse_error", "Cannot parse source", "Check source");
    };
    let query = match Query::new(&language, source_query) {
        Ok(q) => q,
        Err(_) => {
            return sc_agent::error(
                "invalid_query",
                "Invalid tree-sitter query",
                "Check query syntax",
            );
        }
    };
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut result = Vec::new();
    while let Some(found) = matches.next() {
        for capture in found.captures() {
            let node = capture.node;
            result.push(json!({"name": query.capture_names().get(capture.index as usize).copied().unwrap_or(""), "text": node.utf8_text(source.as_bytes()).unwrap_or(""), "start_byte": node.start_byte(), "end_byte": node.end_byte()}));
        }
    }
    json!({"ok": true, "value": result})
}
