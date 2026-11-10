//! Model catalog (REQ-SAG-0101, REQ-SAG-0102, REQ-SAG-0103).

use sc_agent_types::{
    Catalog, ErrorId, MODELS, ModelChoice, ModelId, ReleaseDate, ShortName, TypesError,
};

#[cfg(test)]
fn id(text: &str) -> ModelId {
    text.parse().expect("model id")
}

#[cfg(test)]
fn date(text: &str) -> ReleaseDate {
    text.parse().expect("release date")
}

fn latest(catalog: &Catalog, line: ShortName) -> ModelId {
    catalog.resolve(&ModelChoice::Latest(line))
}

/// Every `MODELS` id round-trips through `Display`, `FromStr` and serde.
#[test]
fn test_known_models_roundtrip() {
    for row in MODELS {
        let model = row.model();
        let text = model.to_string();
        assert_eq!(&text.parse::<ModelId>().expect("parse"), model);
        assert_eq!(
            serde_json::to_value(model).expect("serialize"),
            serde_json::json!(text)
        );
        assert_eq!(
            &serde_json::from_value::<ModelId>(serde_json::json!(text)).expect("deserialize"),
            model
        );
    }
}

/// REQ-SAG-0102: every built-in row carries the model's published release date.
#[test]
fn test_builtin_rows_carry_published_release_dates() {
    let expected = [
        ("jev-1.13.0", ShortName::Jev, "2026-09-15"),
        ("jev-1.14.0", ShortName::Jev, "2026-10-14"),
        ("claude-haiku-5-5", ShortName::Haiku, "2026-10-07"),
        ("claude-sonnet-5-5", ShortName::Sonnet, "2026-09-28"),
        ("claude-opus-5-5", ShortName::Opus, "2026-09-22"),
        ("claude-fable-5-2", ShortName::Fable, "2026-11-09"),
        ("gpt-6-luna", ShortName::Luna, "2026-09-22"),
        ("gpt-6.1-terra", ShortName::Terra, "2026-10-26"),
        ("gpt-6.1-sol", ShortName::Sol, "2026-09-29"),
        ("gpt-6-astra", ShortName::Astra, "2026-09-03"),
    ];
    let actual: Vec<(&str, ShortName, String)> = MODELS
        .iter()
        .map(|row| (row.model().as_str(), row.line(), row.released().to_string()))
        .collect();
    let expected: Vec<(&str, ShortName, String)> = expected
        .iter()
        .map(|(model, line, released)| (*model, *line, (*released).to_owned()))
        .collect();
    assert_eq!(actual, expected);
    assert!(
        MODELS
            .iter()
            .all(|row| row.model().as_str() != "claude-fable-5-1")
    );
}

/// Each `Latest` slot of `builtin()` is the newest `MODELS` row of its line, and every line
/// has at least one row.
#[test]
fn test_builtin_latest_is_newest_row_of_each_line() {
    let catalog = Catalog::builtin();
    for &line in ShortName::ALL {
        let newest = MODELS
            .iter()
            .filter(|row| row.line() == line)
            .max_by_key(|row| row.released())
            .expect("every line has a built-in row");
        assert_eq!(&latest(&catalog, line), newest.model(), "{line}");
    }
    assert_eq!(latest(&catalog, ShortName::Jev), id("jev-1.14.0"));
}

/// REQ-SAG-0101 AC1: each short name parses and resolves to the newest model of its line
/// across built-in and config rows.
#[test]
fn test_short_names_resolve_to_newest_across_builtin_and_config() {
    let mut catalog = Catalog::builtin();
    for &line in ShortName::ALL {
        catalog = catalog
            .with_config_model(id(&format!("{line}-next")), line, date("2099-01-01"))
            .expect("config row");
    }
    for &line in ShortName::ALL {
        let choice: ModelChoice = line.as_str().parse().expect("short name");
        assert_eq!(choice, ModelChoice::Latest(line));
        assert_eq!(choice.to_string(), line.as_str());
        assert_eq!(
            catalog.resolve(&choice),
            id(&format!("{line}-next")),
            "{line}"
        );
    }
}

/// REQ-SAG-0101 AC2: a specific version resolves to itself, listed in the catalog or not.
#[test]
fn test_version_resolves_unchanged() {
    let catalog = Catalog::builtin();
    for text in ["jev-1.13.0", "claude-haiku-5-5", "some-unlisted-model"] {
        let choice: ModelChoice = text.parse().expect("version");
        assert_eq!(choice, ModelChoice::Version(id(text)));
        assert_eq!(choice.to_string(), text);
        assert_eq!(catalog.resolve(&choice), id(text));
    }
    // An older version of a line stays itself; the line's latest is not substituted.
    assert_ne!(latest(&catalog, ShortName::Jev), id("jev-1.13.0"));
    let error = "".parse::<ModelChoice>().expect_err("empty");
    assert_eq!(error.id(), "types.invalid_model_id");
}

/// REQ-SAG-0102 AC1/AC2: a later config model wins, an equal date also wins, an earlier date
/// changes nothing.
#[test]
fn test_config_model_newest_wins_equal_date_config_wins() {
    let base = Catalog::builtin();
    let builtin_haiku = latest(&base, ShortName::Haiku);
    assert_eq!(builtin_haiku, id("claude-haiku-5-5"));

    let later = base
        .clone()
        .with_config_model(id("claude-haiku-6-0"), ShortName::Haiku, date("2027-01-01"))
        .expect("later");
    assert_eq!(latest(&later, ShortName::Haiku), id("claude-haiku-6-0"));
    assert_eq!(
        latest(&later, ShortName::Luna),
        latest(&base, ShortName::Luna)
    );

    let equal = base
        .clone()
        .with_config_model(
            id("claude-haiku-5-5-b"),
            ShortName::Haiku,
            date("2026-10-07"),
        )
        .expect("equal");
    assert_eq!(latest(&equal, ShortName::Haiku), id("claude-haiku-5-5-b"));

    let earlier = base
        .clone()
        .with_config_model(id("claude-haiku-4"), ShortName::Haiku, date("2000-01-01"))
        .expect("earlier");
    assert_eq!(latest(&earlier, ShortName::Haiku), builtin_haiku);
    // The earlier model is still in the catalog.
    assert_eq!(earlier.line_of("claude-haiku-4"), Some(ShortName::Haiku));
}

/// A config id cannot move to another line; repeating it under its own line is fine.
#[test]
fn test_model_on_two_lines_is_rejected() {
    let base = Catalog::builtin();
    let error = base
        .clone()
        .with_config_model(
            id("claude-haiku-5-5"),
            ShortName::Sonnet,
            date("2099-01-01"),
        )
        .expect_err("redeclared");
    assert_eq!(error.id(), "types.model_on_two_lines");
    assert_eq!(
        error,
        TypesError::ModelOnTwoLines {
            model: id("claude-haiku-5-5"),
            line: ShortName::Sonnet,
            other_line: ShortName::Haiku,
        }
    );
    let first = base
        .clone()
        .with_config_model(id("shared"), ShortName::Sonnet, date("2099-01-01"))
        .expect("first");
    assert_eq!(
        first
            .with_config_model(id("shared"), ShortName::Haiku, date("2099-01-01"))
            .expect_err("second")
            .id(),
        "types.model_on_two_lines"
    );
    let again = base
        .with_config_model(id("claude-haiku-5-5"), ShortName::Haiku, date("2000-01-01"))
        .expect("same line");
    assert_eq!(again.line_of("claude-haiku-5-5"), Some(ShortName::Haiku));
    assert_eq!(latest(&again, ShortName::Haiku), id("claude-haiku-5-5"));
}

/// `line_of` answers for built-in and config ids, with a `&ModelId` or a plain `&str`.
#[test]
fn test_line_of_takes_model_id_or_str() {
    let catalog = Catalog::builtin()
        .with_config_model(id("gpt-7-luna"), ShortName::Luna, date("2027-01-01"))
        .expect("config row");
    for row in MODELS {
        assert_eq!(catalog.line_of(row.model()), Some(row.line()));
        assert_eq!(catalog.line_of(row.model().as_str()), Some(row.line()));
    }
    assert_eq!(catalog.line_of(&id("gpt-7-luna")), Some(ShortName::Luna));
    assert_eq!(catalog.line_of("gpt-7-luna"), Some(ShortName::Luna));
    assert_eq!(catalog.line_of("not-listed"), None);
    let model = id("claude-opus-5-5");
    assert_eq!(model.as_ref(), "claude-opus-5-5");
}

/// `ModelId` parse rejections carry `types.invalid_model_id`.
#[test]
fn test_model_id_rejections() {
    for bad in ["", "claude\nhaiku", "tab\there", "\u{7f}"] {
        let error = bad.parse::<ModelId>().expect_err(bad);
        assert_eq!(error.id(), "types.invalid_model_id", "{bad:?}");
        assert!(serde_json::from_value::<ModelId>(serde_json::json!(bad)).is_err());
    }
}
