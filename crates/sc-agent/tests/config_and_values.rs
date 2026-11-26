pub mod support;
use sc_agent::{
    Answer, ConfigSources, Configuration, Environment, Secret, digest, error, result, schema, slug,
};
use serde_json::json;
use std::{ffi::OsString, fs};
use support::*;

fn config(
    local: Option<&std::path::Path>,
    global: Option<&std::path::Path>,
    environment: &[(OsString, OsString)],
    project: Option<&str>,
) -> Result<Configuration, sc_agent::Error> {
    ConfigSources {
        local: local.map(Into::into),
        global: global.map(Into::into),
        environment: &Environment(Secret::new(environment.to_vec())),
        project,
    }
    .load()
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
        config(Some(&local), Some(&global), &env, Some("flag"))?
            .config
            .project_required()?,
        "flag"
    );
    assert_eq!(
        config(Some(&local), Some(&global), &env, None)?
            .config
            .project_required()?,
        "environment"
    );
    for (path, expected) in &places {
        let loaded = config(Some(&local), Some(&global), &[], None)?;
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
    let empty = config(Some(&local), Some(&global), &[], None)?;
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
    let loaded = config(None, Some(&root.0), &environment, None)?;
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
    let without_override = config(None, Some(&root.0), &[], None)?;
    assert_eq!(without_override.config.workers, 8);
    assert!(without_override.config.project.is_none());
    assert_eq!(
        fs::read_to_string(local.join("decidr.json"))?,
        r#"{"project":"local","workers":2}"#
    );
    fs::write(root.0.join("decidr.json"), "not JSON")?;
    let bad = config(None, Some(&root.0), &[], None)
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
        schema(
            &json!({"questions":{"q":{"type":answer["type"]}}}),
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

fn configured_models(
    models: serde_json::Value,
) -> Result<sc_agent::Settings, Box<dyn std::error::Error + Send + Sync>> {
    let root = Temp::new()?;
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"models": models}).to_string(),
    )?;
    Ok(config(None, Some(&root.0), &[], None)?.config)
}

#[test]
fn req_sag_0101_configured_models_override_and_extend_the_built_in_table() -> Test {
    let settings = configured_models(
        json!({"jev":{"provider":"typesafe","id":"jev-1.14.0"},"myjev":{"provider":"typesafe","id":"jev-next"}}),
    )?;
    for (name, id) in [("jev", "jev-1.14.0"), ("myjev", "jev-next")] {
        let rows = sc_agent::providers::candidates(name, &settings);
        assert_eq!(rows.len(), 1);
        let row = rows.first().ok_or("configured row absent")?;
        assert_eq!((row.provider.name, row.id), ("typesafe", id));
    }
    let table = sc_agent::providers::table(&settings);
    assert_eq!(
        table
            .iter()
            .skip(settings.models.len())
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        sc_agent::providers::MODELS
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>()
    );
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
    let loaded = config(None, Some(&root.0), &env, None)?;
    let rows = loaded.rows;
    assert!(rows.iter().all(|row| row["path"] != "log_root"));
    std::fs::write(
        root.0.join("decidr.json"),
        json!({"log_root": path}).to_string(),
    )?;
    let failure = config(None, Some(&root.0), &env, None)
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
        let failure = config(None, Some(&root.0), &env, None)
            .err()
            .ok_or("accepted invalid workers")?
            .to_string();
        assert!(failure.contains("DECIDR_WORKERS"), "{failure}");
        assert!(failure.contains("workers"), "{failure}");
        let file = root.0.join("decidr.json");
        fs::write(&file, json!({"workers":value}).to_string())?;
        let failure = config(None, Some(&root.0), &[], None)
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
    let failure = config(None, Some(&root.0), &[], None)
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
    let failure = config(None, Some(&root.0), &env, None)
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
        json!({"workers":2}).to_string(),
    )?;
    let failure = config(Some(&local), Some(&root.0), &[], None)
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
    let loaded = config(None, None, &environment, None)?;
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
        config(None, Some(&root.0), &environment, None)?
            .config
            .project
            .is_none()
    );
    for name in ["sc-agent.json", "decidr.json", "decidr.local.json"] {
        let path = user.join(name);
        let text = r#"{"laya":{"unknown":true}}"#;
        fs::write(&path, text)?;
        let failure = config(None, None, &environment, None)
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
        let failure = config(None, Some(&root.0), &env, None)
            .err()
            .ok_or("non-string env model accepted")?
            .to_string();
        assert!(failure.contains("DECIDR_LAYA__HOST"), "{failure}");
        assert!(failure.contains("laya.host"), "{failure}");
        let mut invalid = runner.clone();
        invalid["host"] = serde_json::from_str(raw)?;
        fs::write(&file, json!({"laya":invalid}).to_string())?;
        let failure = config(None, Some(&root.0), &[], None)
            .err()
            .ok_or("non-string file model accepted")?
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
        let loaded = config(None, Some(&root.0), &env, None)?;
        assert_eq!(
            loaded
                .config
                .local
                .get("laya")
                .ok_or("missing runner")?
                .host,
            if raw == "plain" { "plain" } else { "1" }
        );
    }
    Ok(())
}
