use sc_treesitter::{language, languages, predefined, queries, query};
use serde_json::json;

#[test]
fn req_sag_0302_any_loaded_grammar_uses_the_same_query_surface()
-> Result<(), Box<dyn std::error::Error>> {
    let python = language("python")?;
    let captures = query(&python, "\"é\"", "(string) @literal")?;
    assert_eq!(
        captures,
        json!([{"name":"literal","text":"\"é\"","start_byte":0,"end_byte":4,"start_line":1,"end_line":1}])
    );
    let rust = language("rust")?;
    let captures = query(
        &rust,
        "fn café() {}",
        "(function_item name: (identifier) @name)",
    )?;
    assert_eq!(
        captures,
        json!([{"name":"name","text":"café","start_byte":3,"end_byte":8,"start_line":1,"end_line":1}])
    );
    assert_eq!(query(&python, "x=1", "(string) @literal")?, json!([]));
    Ok(())
}

#[test]
fn req_sag_0303_unknown_languages_queries_and_invalid_query_are_errors()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(languages(), json!(["python", "rust"]));
    assert_eq!(queries("python")?, json!(["branches"]));
    for result in [
        language("absent").map(|_| json!(null)),
        queries("absent"),
        predefined("rust", "absent", "fn f(){}"),
        query(&language("python")?, "x", "("),
    ] {
        let error = result.err().ok_or("invalid query accepted")?;
        let value = serde_json::to_value(error)?;
        assert!(
            value["id"]
                .as_str()
                .ok_or("missing id")?
                .starts_with("treesitter.")
        );
        assert!(
            !value["message"]
                .as_str()
                .ok_or("missing message")?
                .is_empty()
        );
    }
    Ok(())
}
