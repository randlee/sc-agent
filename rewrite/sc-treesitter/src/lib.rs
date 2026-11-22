use serde_json::{Value, json};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

pub fn languages() -> Value {
    json!(["python", "rust"])
}
pub fn language(name: &str) -> Result<Language, String> {
    match name {
        "python" => Ok(tree_sitter_python::LANGUAGE.into()),
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        _ => Err(format!("unknown language {name:?}; use sitr languages")),
    }
}
pub fn queries(name: &str) -> Result<Value, String> {
    language(name)?;
    Ok(if name == "rust" {
        json!(["match-arms"])
    } else {
        json!(["branches"])
    })
}
pub fn query(language: &Language, source: &str, query: &str) -> Result<Value, String> {
    let query = Query::new(language, query).map_err(|e| e.to_string())?;
    let mut parser = Parser::new();
    parser.set_language(language).map_err(|e| e.to_string())?;
    let tree = parser
        .parse(source, None)
        .ok_or("tree-sitter parse failed")?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut captures = Vec::new();
    while let Some(matched) = matches.next() {
        for capture in matched.captures() {
            let name = query
                .capture_names()
                .get(capture.index as usize)
                .ok_or("capture name absent")?;
            let node = capture.node;
            captures.push(json!({"name":name,"text":node.utf8_text(source.as_bytes()).map_err(|e|e.to_string())?,"start_byte":node.start_byte(),"end_byte":node.end_byte(),"start_point":{"row":node.start_position().row,"column":node.start_position().column},"end_point":{"row":node.end_position().row,"column":node.end_position().column}}));
        }
    }
    Ok(json!(captures))
}
pub fn predefined(language_name: &str, name: &str, source: &str) -> Result<Value, String> {
    let text = match (language_name, name) {
        ("rust", "match-arms") => "(match_arm) @arm",
        ("python", "branches") => "[(if_statement) (elif_clause) (else_clause)] @branch",
        _ => return Err(format!("unknown query {name:?}; use sitr queries")),
    };
    query(&language(language_name)?, source, text)
}
