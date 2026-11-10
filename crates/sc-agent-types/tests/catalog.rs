//! Model catalog (REQ-SAG-0147, REQ-SAG-0148, ADR-SAG-0030).

use sc_agent_types::{MODELS, ModelAliases, ModelFamily, ModelId, ModelSource};

/// Every `MODELS` string parses to its known variant, never `Other`, and
/// round-trips through `Display` and serde.
#[test]
fn test_known_models_roundtrip() {
    for row in MODELS {
        let model = row.model();
        assert!(
            !matches!(model, ModelId::Other(_)),
            "{} parsed as Other",
            row.alias()
        );
        let text = model.to_string();
        assert_eq!(text.parse::<ModelId>().expect("parse"), model);
        assert_eq!(
            serde_json::to_value(&model).expect("serialize"),
            serde_json::json!(text)
        );
        assert_eq!(
            serde_json::from_value::<ModelId>(serde_json::json!(text)).expect("deserialize"),
            model
        );
        assert_eq!(model.family(), row.family());
    }
    assert_eq!(ModelId::Jev1_13_0.as_str(), "jev-1.13.0");
    assert_eq!(ModelId::Jev1_14_0.as_str(), "jev-1.14.0");
    assert_eq!(ModelId::ClaudeHaiku5_5.as_str(), "claude-haiku-5-5");
    assert_eq!(ModelId::Gpt6Luna.as_str(), "gpt-6-luna");
}
#[test]
fn test_file_rows_resolve_to_the_newest_released() {
    let base = ModelAliases::builtin();
    let newer = base
        .clone()
        .with_file_model("claude-haiku-9", "haiku", "2099-01-01")
        .expect("row");
    assert_eq!(
        newer.latest("haiku").expect("model").as_str(),
        "claude-haiku-9"
    );
    assert_eq!(newer.source("haiku"), Some(ModelSource::File));
    assert_eq!(newer.source("luna"), Some(ModelSource::Crate));
    // An older row loses to the crate.
    let older = base
        .clone()
        .with_file_model("claude-haiku-4", "haiku", "2000-01-01")
        .expect("row");
    assert_eq!(older.latest("haiku"), Some(ModelId::ClaudeHaiku5_5));
    assert_eq!(older.source("haiku"), Some(ModelSource::Crate));
    // An override stays on top of a file row.
    let pinned = newer
        .with_override("haiku", ModelId::ClaudeHaiku5_5)
        .expect("override")
        .with_file_model("claude-haiku-10", "haiku", "2099-02-02")
        .expect("row");
    assert_eq!(pinned.source("haiku"), Some(ModelSource::Override));
    for (id, alias, released) in [
        ("claude-haiku-9", "nope", "2099-01-01"),
        ("", "haiku", "2099-01-01"),
        ("claude-haiku-9", "haiku", "2099-13-01"),
        ("claude-haiku-9", "haiku", "soon"),
    ] {
        let error = base
            .clone()
            .with_file_model(id, alias, released)
            .expect_err(released);
        assert_eq!(error.code(), "CONFIG_INVALID", "{id} {alias} {released}");
    }
}

/// A file row cannot move a model id to another alias: that would silently
/// stop `--backend claude-haiku-5-5` working. The conflict is `CONFIG_INVALID`
/// (like the file's other errors); repeating the id under its own alias is fine.
#[test]
fn test_file_row_cannot_redeclare_an_id_under_another_alias() {
    let base = ModelAliases::builtin();
    let error = base
        .clone()
        .with_file_model("claude-haiku-5-5", "sonnet", "2099-01-01")
        .expect_err("redeclared");
    assert_eq!(error.code(), "CONFIG_INVALID");
    assert!(error.message().contains("claude-haiku-5-5"), "{error}");
    assert!(error.message().contains("haiku"), "{error}");
    // Two file rows with one id under two aliases conflict too.
    let first = base
        .clone()
        .with_file_model("claude-shared", "sonnet", "2099-01-01")
        .expect("first");
    assert_eq!(
        first
            .with_file_model("claude-shared", "haiku", "2099-01-01")
            .expect_err("second")
            .code(),
        "CONFIG_INVALID"
    );
    // The same alias again is not a conflict, and the catalog is unchanged.
    let again = base
        .with_file_model("claude-haiku-5-5", "haiku", "2000-01-01")
        .expect("same alias");
    assert_eq!(again.alias_of("claude-haiku-5-5"), Some("haiku"));
}
#[test]
fn test_latest_alias_lookup() {
    let aliases = ModelAliases::builtin();
    assert_eq!(aliases.latest("jev"), Some(ModelId::Jev1_14_0));
    assert_eq!(aliases.latest("haiku"), Some(ModelId::ClaudeHaiku5_5));
    assert_eq!(aliases.latest("luna"), Some(ModelId::Gpt6Luna));
    assert_eq!(aliases.latest("nope"), None);
    for row in MODELS {
        let date = row.released();
        let parts: Vec<&str> = date.split('-').collect();
        assert_eq!(parts.len(), 3, "{date}");
        let numbers: Vec<u32> = parts
            .iter()
            .map(|part| part.parse().expect("digits"))
            .collect();
        assert_eq!(parts[0].len(), 4, "{date}");
        assert_eq!(parts[1].len(), 2, "{date}");
        assert_eq!(parts[2].len(), 2, "{date}");
        assert!((1..=12).contains(&numbers[1]), "{date}");
        assert!((1..=31).contains(&numbers[2]), "{date}");
        assert!(aliases.latest(row.alias()).is_some());
    }
}
/// `jev` is accepted as the latest Jev model (agt-cac) and is not a `MODELS`
/// row, so it never becomes the `jev` alias default.
#[test]
fn test_bare_jev_is_the_latest_model() {
    let model: ModelId = "jev".parse().expect("jev");
    assert_eq!(model, ModelId::Jev);
    assert_eq!(model.as_str(), "jev");
    assert_eq!(model.family(), ModelFamily::TypeSafeJev);
    assert_eq!(
        serde_json::from_value::<ModelId>(serde_json::json!("jev")).expect("serde"),
        ModelId::Jev
    );
    assert!(MODELS.iter().all(|row| row.model().as_str() != "jev"));
    assert_eq!(
        ModelAliases::builtin().latest("jev"),
        Some(ModelId::Jev1_14_0)
    );
}
