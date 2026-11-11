//! Name types: ShortName, Provider, ReleaseDate, VarName, SessionId (REQ-SAG-0001,
//! REQ-SAG-0013, REQ-SAG-0103).

use sc_agent_types::{
    Catalog, ErrorId, MODELS, ModelChoice, ModelId, Provider, ReleaseDate, SessionId, ShortName,
    VarName,
};

#[test]
fn test_short_name_documented_spelling() {
    let spellings = [
        "jev", "haiku", "sonnet", "opus", "fable", "luna", "terra", "sol", "astra",
    ];
    let names: Vec<&str> = ShortName::ALL.iter().map(|name| name.as_str()).collect();
    assert_eq!(names, spellings);
    for &name in ShortName::ALL {
        assert_eq!(
            serde_json::to_value(name).expect("serialize"),
            name.as_str()
        );
        assert_eq!(name.to_string().parse::<ShortName>().expect("parse"), name);
        assert_eq!(
            serde_json::from_value::<ShortName>(serde_json::json!(name.as_str()))
                .expect("deserialize"),
            name
        );
    }
    for bad in ["", "Haiku", "HAIKU", "claude-haiku", "nope"] {
        let error = bad.parse::<ShortName>().expect_err(bad);
        assert_eq!(error.id(), "types.unknown_short_name", "{bad:?}");
        assert!(serde_json::from_value::<ShortName>(serde_json::json!(bad)).is_err());
    }
}

#[test]
fn test_provider_documented_spelling() {
    assert_eq!(Provider::OpenAi.as_str(), "openai");
    assert_eq!(
        serde_json::to_value(Provider::OpenAi).expect("key"),
        "openai"
    );
    assert_eq!(
        serde_json::to_value(Provider::TypeSafe).expect("key"),
        "typesafe"
    );
    let spellings: Vec<&str> = Provider::ALL
        .iter()
        .map(|provider| provider.as_str())
        .collect();
    assert_eq!(
        spellings,
        [
            "typesafe",
            "anthropic",
            "openai",
            "openrouter",
            "nous",
            "together"
        ]
    );
    for &provider in Provider::ALL {
        assert_eq!(
            serde_json::to_value(provider).expect("provider"),
            provider.to_string()
        );
        assert_eq!(
            provider.to_string().parse::<Provider>().expect("parse"),
            provider
        );
    }
    for bad in ["", "open_ai", "OpenAI", "codex", "open-router"] {
        let error = bad.parse::<Provider>().expect_err(bad);
        assert_eq!(error.id(), "types.unknown_provider", "{bad:?}");
    }
}

/// REQ-SAG-0103: PRIORITY is the fixed ranking and `ranking(line)` lists the providers that
/// serve a line in that order.
#[test]
fn test_provider_priority_and_ranking() {
    use Provider::{Anthropic, Nous, OpenAi, OpenRouter, Together, TypeSafe};
    assert_eq!(
        Provider::PRIORITY,
        [TypeSafe, Anthropic, OpenAi, OpenRouter, Together, Nous]
    );
    let expected = |line: Option<ShortName>| -> &'static [Provider] {
        match line {
            Some(ShortName::Jev) => &[TypeSafe],
            Some(ShortName::Haiku | ShortName::Sonnet | ShortName::Opus | ShortName::Fable) => {
                &[Anthropic, OpenRouter, Together]
            }
            Some(_) => &[OpenAi, OpenRouter, Together],
            None => &[OpenRouter, Together, Nous],
        }
    };
    let lines = ShortName::ALL.iter().copied().map(Some).chain([None]);
    for line in lines {
        let ranking = Provider::ranking(line);
        assert_eq!(ranking, expected(line), "{line:?}");
        let positions: Vec<usize> = ranking
            .iter()
            .map(|provider| {
                Provider::PRIORITY
                    .iter()
                    .position(|ranked| ranked == provider)
                    .expect("ranked provider")
            })
            .collect();
        assert!(positions.is_sorted(), "{line:?} not in PRIORITY order");
    }
    for name in ["luna", "terra", "sol", "astra"] {
        let line: ShortName = name.parse().expect("line");
        assert_eq!(
            Provider::ranking(Some(line)),
            &[OpenAi, OpenRouter, Together]
        );
    }
}

/// REQ-SAG-0103: a catalog ModelId encodes no provider and is used with two providers
/// unchanged.
#[test]
fn test_model_id_is_provider_neutral() {
    for row in MODELS {
        for &provider in Provider::ALL {
            assert!(
                !row.model().as_str().contains(provider.as_str()),
                "{} names {provider}",
                row.model()
            );
        }
    }
    let catalog = Catalog::builtin();
    let model = catalog.resolve(&ModelChoice::Latest(ShortName::Haiku));
    let ranking = Provider::ranking(catalog.line_of(&model));
    let requests: Vec<(Provider, ModelId)> = ranking
        .iter()
        .take(2)
        .map(|&provider| (provider, model.clone()))
        .collect();
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0].0, requests[1].0);
    assert_eq!(requests[0].1, requests[1].1);
    assert_eq!(requests[1].1.as_str(), "claude-haiku-5-5");
}

#[test]
fn test_release_date_spelling_and_order() {
    let date: ReleaseDate = "2026-09-15".parse().expect("date");
    assert_eq!(date.to_string(), "2026-09-15");
    assert_eq!(serde_json::to_value(date).expect("serialize"), "2026-09-15");
    assert_eq!(
        serde_json::from_value::<ReleaseDate>(serde_json::json!("2026-09-15")).expect("serde"),
        date
    );
    // An ordering key, not a calendar: day 31 is accepted in any month.
    assert!("2026-02-31".parse::<ReleaseDate>().is_ok());
    let order: Vec<ReleaseDate> = ["2025-12-31", "2026-01-01", "2026-01-02", "2026-10-01"]
        .iter()
        .map(|text| text.parse().expect("date"))
        .collect();
    assert!(order.is_sorted());
    for bad in [
        "",
        "soon",
        "2026-13-01",
        "2026-00-10",
        "2026-01-00",
        "2026-01-32",
        "2026-1-01",
        "2026/01/01",
        "2026-01-011",
        "+026-01-01",
        "2026-0a-01",
    ] {
        let error = bad.parse::<ReleaseDate>().expect_err(bad);
        assert_eq!(error.id(), "types.invalid_release_date", "{bad:?}");
        assert!(serde_json::from_value::<ReleaseDate>(serde_json::json!(bad)).is_err());
    }
}

/// REQ-SAG-0013 (key_env pattern part): `^[A-Z_][A-Z0-9_]{0,127}$`.
#[test]
fn test_var_name_pattern() {
    let longest = format!("A{}", "B".repeat(127));
    for good in [
        "MY_RUNNER_TOKEN",
        "_",
        "A",
        "_X1",
        "X_1_2",
        longest.as_str(),
    ] {
        let name: VarName = good.parse().expect(good);
        assert_eq!(name.as_str(), good);
        assert_eq!(name.as_ref(), good);
        assert_eq!(name.to_string(), good);
        assert_eq!(serde_json::to_value(&name).expect("serialize"), good);
        assert_eq!(
            serde_json::from_value::<VarName>(serde_json::json!(good)).expect("serde"),
            name
        );
    }
    let too_long = format!("A{}", "B".repeat(128));
    assert_eq!(too_long.len(), 129);
    for bad in ["", "lower", "1X", "A-B", "A B", "ÄB", too_long.as_str()] {
        let error = bad.parse::<VarName>().expect_err(bad);
        assert_eq!(error.id(), "types.invalid_var_name", "{bad:?}");
        assert!(serde_json::from_value::<VarName>(serde_json::json!(bad)).is_err());
    }
}

/// REQ-SAG-0001: `SessionId` is a ULID in `Display`, `FromStr` and serde; a UUID is refused
/// with `types.invalid_session_id`.
#[test]
fn test_session_id_is_ulid() {
    let id = SessionId::new();
    let text = id.to_string();
    assert_eq!(text.len(), 26);
    assert!(
        text.bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase())
    );
    assert_eq!(text.parse::<SessionId>().expect("display roundtrip"), id);
    let json = serde_json::to_string(&id).expect("serialize");
    assert_eq!(json, format!("\"{text}\""));
    assert_eq!(
        serde_json::from_str::<SessionId>(&json).expect("deserialize"),
        id
    );
    assert_ne!(SessionId::new(), SessionId::new());

    let uuid_text = "3f2b8a9e-6c1d-4e7a-9b25-0d4c1a7e5f10";
    let lowercase = "01arz3ndektsv4rrffq69g5fav";
    for bad in [
        uuid_text,
        lowercase,
        "",
        "nope",
        &"0".repeat(25),
        &"Z".repeat(26),
    ] {
        let error = bad.parse::<SessionId>().expect_err("rejected");
        assert_eq!(error.id(), "types.invalid_session_id", "{bad:?}");
        assert!(error.to_string().contains("ULID"), "{error}");
        assert!(
            serde_json::from_value::<SessionId>(serde_json::json!(bad)).is_err(),
            "{bad:?}"
        );
    }
}
