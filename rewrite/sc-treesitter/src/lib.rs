use serde_json::{Value, json};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use sc_agent::error;
pub fn languages() -> Value {
    json!(["python", "rust"])
}
pub fn language(name: &str) -> Result<Language, sc_agent::Error> {
    match name {
        "python" => Ok(tree_sitter_python::LANGUAGE.into()),
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        _ => Err(error(
            "treesitter.unknown_language",
            format!("unknown language {name:?}; use sitr languages"),
        )),
    }
}
pub fn queries(name: &str) -> Result<Value, sc_agent::Error> {
    language(name)?;
    Ok(if name == "rust" {
        json!(["match-arms"])
    } else {
        json!(["branches"])
    })
}
pub fn query(language: &Language, source: &str, query: &str) -> Result<Value, sc_agent::Error> {
    let query = Query::new(language, query).map_err(|e| error("treesitter.query_compile", e))?;
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .map_err(|e| error("treesitter.parse", e))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| error("treesitter.parse", "tree-sitter parse failed"))?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut captures = Vec::new();
    while let Some(matched) = matches.next() {
        for capture in matched.captures() {
            let name = query
                .capture_names()
                .get(capture.index as usize)
                .ok_or_else(|| error("treesitter.capture", "capture name absent"))?;
            let node = capture.node;
            captures.push(json!({"name":name,"text":node.utf8_text(source.as_bytes()).map_err(|e|error("treesitter.capture",e))?,"start_byte":node.start_byte(),"end_byte":node.end_byte(),"start_line":node.start_position().row+1,"end_line":node.end_position().row+1}));
        }
    }
    Ok(json!(captures))
}
pub fn predefined(language_name: &str, name: &str, source: &str) -> Result<Value, sc_agent::Error> {
    let text = match (language_name, name) {
        ("rust", "match-arms") => "(match_arm pattern: (_) @arm)",
        ("python", "branches") => "(case_clause (case_pattern) @arm)",
        _ => {
            return Err(error(
                "treesitter.unknown_query",
                format!("unknown query {name:?}; use sitr queries"),
            ));
        }
    };
    query(&language(language_name)?, source, text)
}
