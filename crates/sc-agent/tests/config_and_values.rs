pub mod support;
use sc_agent::{Answer, config, digest, error, key, model_with, result, schema, slug};
use serde_json::json;
use std::{cell::RefCell, ffi::OsString, fs};
use support::*;

#[test]
fn req_sag_0401_env_dotenv_keyring_order_and_invalid_credentials() -> Test {
    let root = Temp::new()?;
    let calls = RefCell::new(vec![]);
    let lookup = |name: &str| {
        calls.borrow_mut().push(name.to_owned());
        Ok("ring-key".into())
    };
    let environment = vec![(
        OsString::from("TYPESAFE_API_KEY"),
        OsString::from("env-key"),
    )];
    fs::write(root.0.join(".env"), "TYPESAFE_API_KEY=dotenv-key\n")?;
    assert_eq!(
        key("TYPESAFE_API_KEY", &environment, &root.0, &lookup)?,
        "env-key"
    );
    assert_eq!(
        key("TYPESAFE_API_KEY", &[], &root.0, &lookup)?,
        "dotenv-key"
    );
    assert!(calls.borrow().is_empty());
    fs::remove_file(root.0.join(".env"))?;
    assert_eq!(key("TYPESAFE_API_KEY", &[], &root.0, &lookup)?, "ring-key");
    assert_eq!(*calls.borrow(), vec!["TYPESAFE_API_KEY"]);
    for value in ["", "with space", "line\nend", "control\u{7f}", "nonascii-é"] {
        let env = vec![("TYPESAFE_API_KEY".into(), value.into())];
        let got = key("TYPESAFE_API_KEY", &env, &root.0, &lookup);
        let error = got.err().ok_or("invalid credential accepted")?;
        let expected = if value.is_empty() {
            "secrets.key_missing"
        } else {
            "secrets.key_malformed"
        };
        assert_eq!(serde_json::to_value(error)?["id"], expected);
    }
    assert_eq!(calls.borrow().len(), 1);
    let failure = key("TYPESAFE_API_KEY", &[], &root.0, &|_| {
        Err(sc_agent::KeyringError::Read("controlled refusal".into()))
    })
    .err()
    .ok_or("keyring failure lost")?;
    let failure = serde_json::to_value(failure)?;
    assert_eq!(failure["id"], "secrets.keyring_read");
    let message = failure["message"]
        .as_str()
        .ok_or("keyring failure message absent")?;
    assert!(message.contains("platform keyring is unavailable"));
    assert!(message.contains("TYPESAFE_API_KEY"));
    assert!(message.contains("environment"));
    assert!(message.contains(".env"));
    assert!(message.contains("controlled refusal"));
    Ok(())
}

#[test]
fn req_sag_0004_slug_validation_never_rewrites_invalid_values() -> Test {
    for valid in ["phase-b", "rust_developer", "a.0", "z"] {
        slug(valid)?;
    }
    for invalid in ["", ".", "..", "Phase B", "../escape", "back\\slash", "é"] {
        let got = result(slug(invalid).map(|_| json!(null)));
        assert_eq!(got["error"]["id"], "log.invalid_slug", "{invalid}");
    }
    Ok(())
}

#[test]
fn req_sag_0004_config_layers_sources_and_required_project() -> Test {
    // Flag > SC_PROJECT > local override/CLI/shared > user override/CLI/shared > default.
    let root = Temp::new()?;
    let local = root.0.join("local");
    let global = root.0.join("global");
    fs::create_dir(&local)?;
    fs::create_dir(&global)?;
    let places = [
        (local.join("decidr.local.json"), "local-override"),
        (local.join("decidr.json"), "local-cli"),
        (local.join("sc-agent.json"), "local-shared"),
        (global.join("decidr.local.json"), "global-override"),
        (global.join("decidr.json"), "global-cli"),
        (global.join("sc-agent.json"), "global-shared"),
    ];
    for (path, project) in &places {
        fs::write(path, json!({"project":project}).to_string())?;
    }
    let env = vec![(OsString::from("SC_PROJECT"), OsString::from("environment"))];
    assert_eq!(
        config("decidr", Some(&local), Some(&global), &env, Some("flag"))?
            .config
            .project_required()?,
        "flag"
    );
    assert_eq!(
        config("decidr", Some(&local), Some(&global), &env, None)?
            .config
            .project_required()?,
        "environment"
    );
    for (path, expected) in &places {
        let loaded = config("decidr", Some(&local), Some(&global), &[], None)?;
        assert_eq!(loaded.config.project_required()?, *expected);
        let resolved = loaded.rows;
        assert_eq!(
            resolved
                .iter()
                .find(|row| row["path"] == "project")
                .ok_or("project source absent")?["source"],
            path.to_string_lossy().as_ref()
        );
        fs::remove_file(path)?;
    }
    let empty = config("decidr", Some(&local), Some(&global), &[], None)?;
    assert_eq!(empty.config.workers, 8);
    let error = empty
        .config
        .project_required()
        .err()
        .ok_or("missing project accepted")?;
    for place in [
        "--project",
        "SC_PROJECT",
        "local decidr.json",
        "global sc-agent.json",
    ] {
        assert!(error.to_string().contains(place));
    }
    Ok(())
}

#[test]
fn req_sag_0004_config_injected_env_nested_keys_and_outside_git() -> Test {
    // ADR-SAG-0020: DECIDR_ prefix and double underscores; explicitly skip local files outside git.
    let root = Temp::new()?;
    let local = root.0.join("local");
    fs::create_dir(&local)?;
    fs::write(
        local.join("decidr.json"),
        r#"{"project":"local","workers":2}"#,
    )?;
    let environment = [
        ("SC_PROJECT", "env"),
        ("DECIDR_WORKERS", "3"),
        ("DECIDR_LAYA__HOST", "example.invalid"),
        ("DECIDR_LAYA__PORT", "9000"),
    ]
    .map(|(k, v)| (k.into(), v.into()));
    let loaded = config("decidr", None, Some(&root.0), &environment, None)?;
    assert_eq!(loaded.config.workers, 3);
    let runner = loaded
        .config
        .local
        .get("laya")
        .ok_or("runner not decoded")?;
    assert_eq!(
        (runner.host.as_str(), runner.port),
        ("example.invalid", 9000)
    );
    let without_override = config("decidr", None, Some(&root.0), &[], None)?;
    assert_eq!(without_override.config.workers, 8);
    assert!(without_override.config.project.is_none());
    assert_eq!(
        fs::read_to_string(local.join("decidr.json"))?,
        r#"{"project":"local","workers":2}"#
    );
    fs::write(root.0.join("decidr.json"), "not JSON")?;
    let bad = config("decidr", None, Some(&root.0), &[], None)
        .err()
        .ok_or("bad config accepted")?;
    assert!(bad.to_string().contains("decidr.json"));
    Ok(())
}

#[test]
fn req_sag_0202_digest_includes_all_information_and_exactly_one_question() -> Test {
    let mut original = json!({"state":{"nested":[1,"evidence"]},"context":"all info","questions":{"q":{"type":"noul"},"other":{"type":"box"}}});
    let expected = json!({"state":{"nested":[1,"evidence"]},"context":"all info","questions":{"q":{"type":"noul"}}});
    let bytes = serde_json::to_vec(&expected)?;
    let hash = sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
        utf8_file_bytes: &bytes,
    })?;
    let before = digest(&original, "q")?;
    assert_eq!(before, hash.template().to_string());
    original["questions"]["other"] = json!({"type":"score"});
    assert_eq!(before, digest(&original, "q")?);
    original["context"] = json!("changed");
    assert_ne!(before, digest(&original, "q")?);
    original["context"] = json!("all info");
    original["questions"]["q"]["instructions"] = json!("changed question");
    assert_ne!(before, digest(&original, "q")?);
    assert!(digest(&original, "absent").is_err());
    assert!(digest(&json!(null), "q").is_err());
    Ok(())
}

#[test]
fn req_sag_0007_0201_distinct_answer_types_and_schema_failures() -> Test {
    for answer in [
        json!({"type":"noul","noul":0.123456789}),
        json!({"type":"choice","choice":"yes","confidence":0.73123456789,"probabilities":{"yes":0.73123456789}}),
        json!({"type":"score","score":0.123456789,"confidence":0.8,"legend":{"0":"low"},"probabilities":{"0":0.123456789}}),
        json!({"type":"box","box":{"top":0.123456789,"bottom":0.9,"left":0.2,"right":0.8},"confidence":null}),
        json!({"type":"box","box":null,"confidence":null}),
    ] {
        let typed: Answer = serde_json::from_value(answer.clone())?;
        assert_eq!(serde_json::to_value(typed)?, answer);
        let mut question = json!({"type":answer["type"]});
        match answer["type"].as_str() {
            Some("choice") => question["criteria"] = json!({"yes":"true"}),
            Some("score") => question["criteria"] = json!(["low"]),
            _ => {}
        }
        schema(
            &json!({"questions":{"q":question}}),
            &json!({"answers":{"q":answer}}),
        )?;
    }
    for bad in [
        json!({}),
        json!({"answers":[]}),
        json!({"answers":{"other":{"type":"noul","noul":0.1}}}),
        json!({"answers":{"q":{"type":"noul"}}}),
        json!({"answers":{"q":{"type":"unknown"}}}),
        json!({"answers":{"q":{"type":"box","box":null,"confidence":null}}}),
    ] {
        let error = schema(&request(), &bad)
            .err()
            .ok_or("invalid response accepted")?;
        assert_eq!(
            serde_json::to_value(error)?["id"],
            "decision.schema_mismatch"
        );
    }
    assert!(schema(&json!({}), &answer()).is_err());
    Ok(())
}

#[test]
fn req_sag_0002_0016_0017_errors_have_stable_ids_categories_and_omit_unset_attempts() -> Test {
    for (id, category) in [
        ("http.network", "network"),
        ("http.provider_busy", "busy"),
        ("http.auth", "auth"),
        ("http.quota_exhausted", "quota"),
        ("decision.cancelled", "cancelled"),
        ("log.flush", "log"),
        ("config.invalid", "config"),
        ("decision.schema_mismatch", "response_invalid"),
        ("decision.request_parse", "validation"),
    ] {
        let value = result(Err(error(id, "detail remains whole")));
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["id"], id);
        assert_eq!(value["error"]["message"], "detail remains whole");
        assert_eq!(value["error"]["category"], category);
        assert!(value["error"].get("attempts").is_none());
    }
    assert_eq!(
        result(Ok(json!({"answer":0.123456789}))),
        json!({"ok":true,"value":{"answer":0.123456789}})
    );
    Ok(())
}

#[test]
fn req_sag_0101_models_use_provider_tables_and_reseller_passthrough() -> Test {
    for (name, provider, id) in [
        ("jev", None, "jev-latest"),
        ("haiku", None, "claude-haiku-5-5"),
        ("gpt-6-luna", None, "gpt-6-luna"),
        ("haiku", Some("openrouter"), "anthropic/claude-haiku-5.5"),
        ("custom/id", Some("nous"), "custom/id"),
        ("future-model", Some("together"), "future-model"),
    ] {
        let configured = provider
            .map(|provider| {
                (
                    name.into(),
                    sc_agent::ModelEntry {
                        provider: provider.into(),
                        id: id.into(),
                        released: None,
                    },
                )
            })
            .into_iter()
            .collect();
        assert_eq!(model_with(name, &configured)?.1, id);
    }
    assert_eq!(model_with("laya", &Default::default())?.0.name, "laya");
    assert_eq!(
        model_with("decider", &Default::default())?.1,
        "decider-4b-v2.1"
    );
    assert!(model_with("missing", &Default::default()).is_err());
    assert!(
        model_with(
            "jev",
            &[(
                "jev".into(),
                sc_agent::ModelEntry {
                    provider: "missing".into(),
                    id: "unused".into(),
                    released: None
                }
            )]
            .into()
        )
        .is_err()
    );
    Ok(())
}

fn configured_models(
    models: serde_json::Value,
) -> Result<sc_agent::Settings, Box<dyn std::error::Error + Send + Sync>> {
    let root = Temp::new()?;
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"models": models}).to_string(),
    )?;
    Ok(config("decidr", None, Some(&root.0), &[], None)?.config)
}

#[test]
fn req_sag_0101_configured_models_override_and_extend_the_built_in_table() -> Test {
    let none = configured_models(json!({}))?;
    let (provider, id) = model_with("jev", &none.models)?;
    assert_eq!((provider.name, id.as_str()), ("typesafe", "jev-latest"));
    let pinned = configured_models(json!({
        "jev": {"provider": "typesafe", "id": "jev-1.14.0"},
        "myjev": {"provider": "typesafe", "id": "jev-next"}
    }))?;
    let (provider, id) = model_with("jev", &pinned.models)?;
    assert_eq!((provider.name, id.as_str()), ("typesafe", "jev-1.14.0"));
    let (provider, id) = model_with("myjev", &pinned.models)?;
    assert_eq!((provider.name, id.as_str()), ("typesafe", "jev-next"));
    Ok(())
}

#[test]
fn req_sag_0019_model_entry_accepts_released_date_and_rejects_malformed_date() -> Test {
    let configured = configured_models(json!({
        "luna": {
            "provider": "nous",
            "id": "openai/gpt-6.1-luna",
            "released": "2026-11-20"
        }
    }))?;
    let model = configured
        .models
        .get("luna")
        .ok_or("missing configured luna")?;
    assert_eq!(model.provider, "nous");
    assert_eq!(model.id, "openai/gpt-6.1-luna");
    assert_eq!(model.released.as_deref(), Some("2026-11-20"));

    let failure = configured_models(json!({
        "luna": {
            "provider": "nous",
            "id": "openai/gpt-6.1-luna",
            "released": "2026-02-30"
        }
    }))
    .err()
    .ok_or("invalid release date accepted")?;
    let message = failure.to_string();
    assert!(message.contains("config.invalid"), "{message}");
    let parser_detail = message
        .split_once("released must be a YYYY-MM-DD date: ")
        .map(|(_, detail)| detail)
        .ok_or("released date error lost its key-specific message")?;
    assert!(!parser_detail.is_empty(), "{message}");

    let failure = configured_models(json!({
        "luna": {
            "provider": "nous",
            "id": "openai/gpt-6.1-luna",
            "released": null
        }
    }))
    .err()
    .ok_or("null release date accepted")?;
    assert!(failure.to_string().contains("config.invalid"));
    Ok(())
}

#[test]
fn configured_release_dates_keep_calendar_and_format_boundaries() -> Test {
    for (date, valid) in [
        ("2000-02-29", true),
        ("2024-02-29", true),
        ("0000-02-29", true),
        ("9999-12-31", true),
        ("1900-02-29", false),
        ("2025-02-29", false),
        ("2026-04-31", false),
        ("2026-00-01", false),
        ("2026-01-00", false),
        ("2026-1-01", false),
        ("2026-01-1", false),
        ("026-01-01", false),
        ("+2026-01-01", false),
        (" 2026-01-01", false),
        ("2026-01-01T00:00:00Z", false),
    ] {
        let result = configured_models(json!({"luna": {
            "provider":"nous", "id":"openai/gpt-6.1-luna", "released":date
        }}));
        assert_eq!(result.is_ok(), valid, "release date {date}");
        if let Err(error) = result {
            assert!(error.to_string().contains("config.invalid"), "{error}");
        }
    }
    Ok(())
}

#[test]
fn req_sag_0101_configured_model_with_unknown_provider_is_rejected() -> Test {
    let failure = configured_models(json!({"x": {"provider": "nowhere", "id": "x-1"}}))
        .err()
        .ok_or("unknown provider accepted")?;
    assert!(failure.to_string().contains("types.unknown_provider"));
    Ok(())
}

#[test]
fn req_sag_0004_repository_identity_normalizes_origins_and_local_fallback() -> Test {
    for origin in [
        "https://github.com/randlee/sc-agent.git",
        "git@github.com:randlee/sc-agent.git",
        "ssh://git@github.com:2222/randlee/sc-agent.git",
        "https://github.com:8443/randlee/sc-agent.git/",
    ] {
        assert_eq!(
            sc_agent::repository_identity(Some(origin), "m5", std::path::Path::new("work"))?,
            "github.com/randlee/sc-agent"
        );
    }
    assert_eq!(
        sc_agent::repository_identity(None, "m5", std::path::Path::new("work"))?,
        "local/m5/work"
    );
    assert!(
        sc_agent::repository_identity(
            Some("https://host/../escape"),
            "m5",
            std::path::Path::new("work")
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn req_sag_0702_nonpositive_worker_counts_are_typed_errors() -> Test {
    for workers in [-1, 0] {
        let got = sc_agent::result(sc_agent::workers(workers).map(|workers| json!(workers)));
        assert_eq!(got["error"]["id"], "decidr.invalid_workers");
    }
    assert_eq!(sc_agent::workers(1)?, 1);
    assert_eq!(sc_agent::workers(20)?, 20);
    Ok(())
}

#[test]
fn adr_sag_0021_log_root_is_not_a_configuration_setting() -> Test {
    let root = Temp::new()?;
    let path = root.0.join("logs-from-env");
    let env = vec![("DECIDR_LOG_ROOT".into(), path.as_os_str().to_owned())];
    let loaded = config("decidr", None, Some(&root.0), &env, None)?;
    let rows = loaded.rows;
    assert!(rows.iter().all(|row| row["path"] != "log_root"));
    std::fs::write(
        root.0.join("decidr.json"),
        json!({"log_root": path}).to_string(),
    )?;
    let failure = config("decidr", None, Some(&root.0), &env, None)
        .err()
        .ok_or("accepted log_root setting")?;
    assert!(failure.to_string().contains("log_root"));
    assert!(!path.exists());
    Ok(())
}

#[test]
fn req_sag_0019_0020_invalid_values_name_their_key_and_source() -> Test {
    let root = Temp::new()?;
    for value in [json!(0), json!("abc")] {
        let env = vec![(
            "DECIDR_WORKERS".into(),
            value.to_string().trim_matches('"').into(),
        )];
        let failure = config("decidr", None, Some(&root.0), &env, None)
            .err()
            .ok_or("accepted invalid workers")?
            .to_string();
        assert!(failure.contains("DECIDR_WORKERS"), "{failure}");
        assert!(failure.contains("workers"), "{failure}");
        let file = root.0.join("decidr.json");
        fs::write(&file, json!({"workers":value}).to_string())?;
        let failure = config("decidr", None, Some(&root.0), &[], None)
            .err()
            .ok_or("accepted invalid workers file")?
            .to_string();
        assert!(
            failure.contains(file.to_str().ok_or("non-UTF8 path")?),
            "{failure}"
        );
        assert!(failure.contains("workers"), "{failure}");
        fs::remove_file(file)?;
    }
    let file = root.0.join("decidr.json");
    fs::write(&file, json!({"laya":{"host":"localhost"}}).to_string())?;
    let failure = config("decidr", None, Some(&root.0), &[], None)
        .err()
        .ok_or("accepted partial runner")?
        .to_string();
    assert!(
        failure.contains(file.to_str().ok_or("non-UTF8 path")?),
        "{failure}"
    );
    assert!(failure.contains("laya.port"), "{failure}");
    fs::write(
        &file,
        json!({"laya":{"host":"localhost","port":9000}}).to_string(),
    )?;
    let env = vec![("DECIDR_LAYA__PORT".into(), "bad".into())];
    let failure = config("decidr", None, Some(&root.0), &env, None)
        .err()
        .ok_or("accepted invalid port")?
        .to_string();
    assert!(failure.contains("DECIDR_LAYA__PORT"), "{failure}");
    assert!(failure.contains("laya.port"), "{failure}");
    Ok(())
}

#[test]
fn partial_runner_across_layers_names_missing_key_and_all_sources() -> Test {
    let root = Temp::new()?;
    let local = root.0.join("local");
    fs::create_dir(&local)?;
    fs::write(
        local.join("decidr.json"),
        json!({"laya":{"host":"localhost"}}).to_string(),
    )?;
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"laya":{"host":"remote"}}).to_string(),
    )?;
    let failure = config("decidr", Some(&local), Some(&root.0), &[], None)
        .err()
        .ok_or("partial runner accepted")?
        .to_string();
    for term in [
        "laya.port",
        "DECIDR_LAYA__PORT",
        "local decidr.json",
        "local sc-agent.json",
        "global decidr.json",
        "global sc-agent.json",
    ] {
        assert!(failure.contains(term), "{failure}");
    }
    Ok(())
}

#[test]
fn config_home_and_file_acceptance_are_explicit_and_read_only() -> Test {
    let root = Temp::new()?;
    let user = root.0.join("relocated");
    fs::create_dir(&user)?;
    let environment = vec![
        ("SC_AGENT_HOME".into(), user.as_os_str().to_owned()),
        ("DECIDR_PROJECT".into(), "ignored".into()),
        ("DECIDR_LOG_ROOT".into(), "ignored".into()),
    ];
    let file = user.join("decidr.local.json");
    fs::write(&file, r#"{"project":"relocated","workers":3}"#)?;
    // No system/project slot, sibling CLI or shared .local file is read.
    for name in ["sc-agent.local.json", "sitr.json"] {
        fs::write(user.join(name), "invalid JSON must not be read")?;
    }
    let loaded = config("decidr", None, None, &environment, None)?;
    assert_eq!(loaded.config.project_required()?, "relocated");
    assert_eq!(loaded.config.workers, 3);
    assert_eq!(loaded.files.len(), 3);
    assert_eq!(
        loaded
            .rows
            .iter()
            .find(|row| row["path"] == "project")
            .ok_or("project row absent")?["source"],
        file.to_string_lossy().as_ref()
    );
    assert!(
        config("decidr", None, Some(&root.0), &environment, None)?
            .config
            .project
            .is_none()
    );
    for name in ["sc-agent.json", "decidr.json", "decidr.local.json"] {
        let path = user.join(name);
        let text = r#"{"laya":{"unknown":true}}"#;
        fs::write(&path, text)?;
        let failure = config("decidr", None, None, &environment, None)
            .err()
            .ok_or("unknown file key accepted")?;
        let failure = serde_json::to_value(failure)?;
        assert_eq!(failure["id"], "config.invalid");
        let message = failure["message"].as_str().ok_or("missing message")?;
        assert!(
            message.contains(path.to_str().ok_or("non-UTF8 path")?),
            "{message}"
        );
        assert!(message.contains("laya.unknown"), "{message}");
        assert_eq!(fs::read_to_string(&path)?, text);
        fs::remove_file(path)?;
    }
    Ok(())
}

#[test]
fn environment_json_and_strings_share_file_validation() -> Test {
    let root = Temp::new()?;
    let file = root.0.join("decidr.json");
    let runner = json!({"host":"localhost", "port":9000});
    for raw in ["1", "true", "null", "[1]", "{\"nested\":1}"] {
        fs::write(&file, json!({"laya":runner}).to_string())?;
        let env = vec![("DECIDR_LAYA__HOST".into(), raw.into())];
        let failure = config("decidr", None, Some(&root.0), &env, None)
            .err()
            .ok_or("non-string env host accepted")?
            .to_string();
        assert!(failure.contains("DECIDR_LAYA__HOST"), "{failure}");
        assert!(failure.contains("laya.host"), "{failure}");
        let mut invalid = runner.clone();
        invalid["host"] = serde_json::from_str(raw)?;
        fs::write(&file, json!({"laya":invalid}).to_string())?;
        let failure = config("decidr", None, Some(&root.0), &[], None)
            .err()
            .ok_or("non-string file host accepted")?
            .to_string();
        assert!(
            failure.contains(file.to_str().ok_or("non-UTF8 path")?),
            "{failure}"
        );
        assert!(failure.contains("laya.host"), "{failure}");
    }
    fs::write(&file, json!({"laya":runner}).to_string())?;
    for raw in ["plain", "\"1\""] {
        let env = vec![("DECIDR_LAYA__HOST".into(), raw.into())];
        let loaded = config("decidr", None, Some(&root.0), &env, None)?;
        assert_eq!(
            loaded.config.local["laya"].host,
            if raw == "plain" { "plain" } else { "1" }
        );
    }
    Ok(())
}

#[test]
fn max_sends_and_system_prompt_keys_load_default_and_report() -> Test {
    let root = Temp::new()?;
    let defaults = config("decidr", None, Some(&root.0), &[], None)?;
    assert_eq!(defaults.config.max_sends, 5);
    let row = |loaded: &sc_agent::Configuration, path: &str| {
        loaded.rows.iter().find(|row| row["path"] == path).cloned()
    };
    assert_eq!(
        row(&defaults, "max_sends"),
        Some(json!({"path":"max_sends","value":"5","source":"default"}))
    );
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"max_sends":3,"decidr":{"luna-system-prompt":"P"}}).to_string(),
    )?;
    let loaded = config("decidr", None, Some(&root.0), &[], None)?;
    assert_eq!(loaded.config.max_sends, 3);
    assert_eq!(loaded.config.decidr["luna-system-prompt"], "P");
    assert_eq!(
        row(&loaded, "decidr.luna-system-prompt").ok_or("row missing")?["value"],
        "P"
    );
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"max_sends":0}).to_string(),
    )?;
    let failure = config("decidr", None, Some(&root.0), &[], None)
        .err()
        .ok_or("zero accepted")?
        .to_string();
    assert!(
        failure.contains("config.invalid") && failure.contains("max_sends"),
        "{failure}"
    );
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"decidr":{"sonnet-system-prompt":"P"}}).to_string(),
    )?;
    assert!(config("decidr", None, Some(&root.0), &[], None).is_err());
    Ok(())
}

#[test]
fn config_local_endpoints() -> Test {
    let root = Temp::new()?;
    let file = root.0.join("decidr.json");
    fs::write(
        &file,
        json!({"decider_4b":{"host":"h4","port":18001},"decider_2b":{"host":"h2","port":18000}})
            .to_string(),
    )?;
    let loaded = config("decidr", None, Some(&root.0), &[], None)?;
    let ports = loaded
        .config
        .local
        .iter()
        .map(|(n, r)| (n.as_str(), r.host.as_str(), r.port));
    assert_eq!(
        ports.collect::<Vec<_>>(),
        [("decider_2b", "h2", 18000), ("decider_4b", "h4", 18001)]
    );
    let paths = loaded
        .rows
        .iter()
        .map(|row| row["path"].as_str().unwrap_or(""));
    let paths = paths.collect::<Vec<_>>();
    for key in [
        "decider_4b.host",
        "decider_4b.port",
        "decider_2b.host",
        "decider_2b.port",
    ] {
        assert!(paths.contains(&key), "{key}");
    }
    assert!(!paths.contains(&"laya.model"));
    fs::write(
        &file,
        json!({"laya":{"host":"h","port":1,"model":"x"}}).to_string(),
    )?;
    let failure = config("decidr", None, Some(&root.0), &[], None)
        .err()
        .ok_or("laya.model accepted")?
        .to_string();
    assert!(
        failure.contains("config.invalid") && failure.contains("laya.model"),
        "{failure}"
    );
    Ok(())
}

#[test]
fn built_in_rows_are_newest_first() {
    for provider in sc_agent::PROVIDERS {
        assert!(
            provider
                .models
                .windows(2)
                .all(|rows| rows[0].2 >= rows[1].2),
            "{}",
            provider.name
        );
    }
}

#[test]
fn generated_answer_schema_rejects_invalid_answer_fields() -> Test {
    let choice = json!({"type":"choice","choice":"a","confidence":0.875,"probabilities":{"a":0.123456789,"b":0.2}});
    let score = json!({"type":"score","score":0.123456789,"confidence":0.8,"legend":{"0":"low","1":"high"},"probabilities":{"0":0.3,"1":0.4}});
    let bounds = json!({"top":0.1,"bottom":0.9,"left":0.2,"right":0.8});
    let boxed = json!({"type":"box","box":bounds,"confidence":null});
    let noul = json!({"type":"noul","noul":0.875});
    let changed = |original: &serde_json::Value, field: &str, value| {
        let mut answer = original.clone();
        answer[field] = value;
        answer
    };
    for (question, valid, invalid) in [
        (
            json!({"type":"choice","criteria":{"a":"A","b":"B"}}),
            choice.clone(),
            vec![
                changed(&choice, "choice", json!("outside")),
                changed(&choice, "probabilities", json!({})),
                changed(
                    &choice,
                    "probabilities",
                    json!({"a":0.1,"b":0.2,"extra":0.3}),
                ),
                changed(&choice, "extra", json!(true)),
            ],
        ),
        (
            json!({"type":"score","criteria":["low","high"]}),
            score.clone(),
            vec![
                changed(&score, "legend", json!({"0":"low"})),
                changed(&score, "probabilities", json!({"0":0.1})),
            ],
        ),
        (
            json!({"type":"box"}),
            boxed.clone(),
            vec![
                json!({"type":"box","box":null}),
                changed(&boxed, "box", changed(&bounds, "extra", json!(1))),
            ],
        ),
        (
            json!({"type":"noul"}),
            noul.clone(),
            vec![changed(&noul, "extra", json!(true))],
        ),
    ] {
        let request = json!({"questions":{"q":question}});
        schema(
            &request,
            &json!({"model":"reply-metadata","answers":{"q":valid}}),
        )?;
        for answer in invalid {
            let failure = schema(&request, &json!({"answers":{"q":answer}}))
                .err()
                .ok_or_else(|| format!("accepted invalid answer {answer}"))?;
            let failure = serde_json::to_value(failure)?;
            assert_eq!(failure["id"], "decision.schema_mismatch");
            assert!(
                failure["message"]
                    .as_str()
                    .ok_or("no error message")?
                    .contains('q')
            );
        }
    }
    Ok(())
}

#[test]
fn array_shaped_answers_and_boxes_are_schema_mismatches() -> Test {
    let mut accepted = Vec::new();
    for (question, answer) in [
        (json!({"type":"noul"}), json!(["noul", 0.5])),
        (
            json!({"type":"choice","criteria":{"a":"A"}}),
            json!(["choice", "outside", 0.8, {}]),
        ),
        (
            json!({"type":"score","criteria":["low"]}),
            json!(["score",0.5,0.8,{"0":"low"},{"0":1.0}]),
        ),
        (json!({"type":"box"}), json!(["box", null, null])),
        (
            json!({"type":"box"}),
            json!({"type":"box","box":[0.1,0.9,0.2,0.8],"confidence":null}),
        ),
    ] {
        match schema(
            &json!({"questions":{"q":question}}),
            &json!({"answers":{"q":answer}}),
        ) {
            Ok(()) => accepted.push(answer),
            Err(failure) => assert_eq!(
                serde_json::to_value(failure)?["id"],
                "decision.schema_mismatch"
            ),
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted array-shaped values: {accepted:?}"
    );
    Ok(())
}

#[test]
fn configuration_files_environment_and_missing_project_follow_the_tool_name() -> Test {
    let root = Temp::new()?;
    std::fs::write(
        root.0.join("judge-jev.json"),
        r#"{"project":"tool-file","workers":2}"#,
    )?;
    assert_eq!(
        config("judge-jev", None, Some(&root.0), &[], None)?
            .config
            .project
            .as_deref(),
        Some("tool-file")
    );
    let env = vec![
        ("SC_PROJECT".into(), "shared-env".into()),
        ("JUDGE_JEV_WORKERS".into(), "3".into()),
        ("DECIDR_WORKERS".into(), "5".into()),
    ];
    let loaded = config("judge-jev", None, Some(&root.0), &env, None)?.config;
    assert_eq!(loaded.project.as_deref(), Some("shared-env"));
    assert_eq!(loaded.workers, 3);
    let absent = config("another-tool", None, Some(&root.0), &[], None)?.config;
    let failure = absent
        .project_required()
        .err()
        .ok_or("project was invented")?
        .to_string();
    assert!(failure.contains("SC_PROJECT"), "{failure}");
    assert!(failure.contains("another-tool.json"), "{failure}");
    Ok(())
}

#[test]
fn j3_cache_setting() -> Test {
    let root = Temp::new()?;
    let defaults = config("decidr", None, Some(&root.0), &[], None)?;
    assert!(defaults.config.cache);
    let row = defaults
        .rows
        .iter()
        .find(|row| row["path"] == "cache")
        .ok_or("missing cache doctor row")?;
    assert_eq!(row["value"], "true");
    assert_eq!(row["source"], "default");
    let disabled = config(
        "decidr",
        None,
        Some(&root.0),
        &[("DECIDR_CACHE".into(), "false".into())],
        None,
    )?;
    assert!(!disabled.config.cache);
    let invalid = config(
        "decidr",
        None,
        Some(&root.0),
        &[("DECIDR_CACHE".into(), "1".into())],
        None,
    );
    assert_eq!(
        serde_json::to_value(invalid.err().ok_or("invalid cache accepted")?)?["id"],
        "config.invalid"
    );
    Ok(())
}

#[test]
fn j3_missing_tool() -> Test {
    use std::sync::Arc;
    let root = Temp::new()?;
    for (tool, version) in [("", "1"), ("decidr", "")] {
        let args = sc_agent::RunArgs {
            tool: tool.into(),
            tool_version: version.into(),
            agent_role: "decidr".into(),
            config_dir: Some(root.0.clone()),
            ..Default::default()
        };
        let error = sc_agent::Run::new(args, None, Arc::new(sc_agent::Cancel::default()))
            .err()
            .ok_or("missing identity accepted")?;
        assert_eq!(serde_json::to_value(error)?["id"], "decision.missing_tool");
    }
    assert_eq!(std::fs::read_dir(root.0.clone())?.count(), 0);
    Ok(())
}

#[test]
fn j3_log_categories() -> Test {
    for (id, category) in [
        ("log.write", "log"),
        ("log.flush", "log"),
        ("log.shutdown", "log"),
        ("log.session_write", "log"),
        ("session.unreadable", "log"),
        ("log.invalid_slug", "validation"),
        ("log.invalid_event", "validation"),
        ("session.not_found", "validation"),
        ("decision.missing_tool", "validation"),
    ] {
        assert_eq!(
            serde_json::to_value(error(id, "test"))?["category"],
            category,
            "{id}"
        );
    }
    Ok(())
}
