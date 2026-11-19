//! The layered loader only uses temporary files and explicitly supplied environment values.
#![cfg(test)]
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sc_agent_config::{
    AgentConfigError, CliName, DoctorRow, LoadOptions, Source, doctor_rows, load_layered,
};
use sc_agent_types::{ErrorCategory, ErrorId, MAX_SEGMENT_BYTES};
use serde_json::{Value, json};

fn options(root: &Path) -> LoadOptions {
    LoadOptions {
        cli: CliName::Decidr,
        home: root.join("home"),
        worktree: Some(root.join("work")),
        default_log_root: root.join("home").join(".sc-agent"),
        flags: BTreeMap::new(),
        environment: Vec::new(),
    }
}
fn global_cli_file(options: &LoadOptions) -> PathBuf {
    options.home.join(".sc-agent").join(options.cli.file_name())
}

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value.to_string()).unwrap();
}
fn env(options: &mut LoadOptions, name: &str, value: &str) {
    options.environment.push((name.into(), value.into()));
}
fn row(loaded: &sc_agent_config::Loaded, name: &str) -> DoctorRow {
    doctor_rows(loaded)
        .into_iter()
        .find(|r| r.setting == name)
        .unwrap()
}

#[test]
fn every_project_layer_wins_in_order_and_missing_is_typed() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let paths = [
        o.home.join(".sc-agent").join("sc-agent.json"),
        global_cli_file(&o),
        o.worktree
            .as_ref()
            .unwrap()
            .join(".sc")
            .join("sc-agent")
            .join("sc-agent.json"),
        o.worktree
            .as_ref()
            .unwrap()
            .join(".sc")
            .join("sc-agent")
            .join(o.cli.file_name()),
    ];
    for (i, path) in paths.iter().enumerate() {
        write(path, &json!({"project":format!("file-{i}")}));
    }
    env(&mut o, "SC_PROJECT", "env");
    o.flags.insert("project".into(), json!("flag"));
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.config.require_project().unwrap(), "flag");
    assert_eq!(row(&loaded, "project").source, "--project");
    o.flags.clear();
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.config.project(), Some("env"));
    assert_eq!(loaded.sources["project"], Source::Env("SC_PROJECT".into()));
    o.environment.clear();
    for i in (0..4).rev() {
        let loaded = load_layered(&o).unwrap();
        assert_eq!(
            loaded.config.require_project().unwrap(),
            format!("file-{i}")
        );
        assert_eq!(
            row(&loaded, "project").source,
            paths[i].display().to_string()
        );
        fs::remove_file(&paths[i]).unwrap();
    }
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.config.project(), None);
    let error = loaded.config.require_project().unwrap_err();
    assert!(matches!(error, AgentConfigError::ProjectRequired { .. }));
    assert_eq!(error.id(), "config.project_required");
    assert_eq!(error.detail().category, Some(ErrorCategory::Config));
    for place in [
        "--project",
        "SC_PROJECT",
        "sc-agent.json",
        "decidr.json",
        "project/phase",
    ] {
        assert!(error.message().contains(place));
    }
}

#[test]
fn nested_merge_preserves_unmodified_values_and_their_sources() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let global = global_cli_file(&o);
    let local = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(o.cli.file_name());
    write(
        &global,
        &json!({"workers":4,"local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":8000,"model":"local/model"}}}),
    );
    write(
        &local,
        &json!({"workers":2,"local_runners":{"laya":{"request_timeout_secs":900}}}),
    );
    env(&mut o, "DECIDR_LOCAL_RUNNERS__LAYA__PORT", "9000");
    let loaded = load_layered(&o).unwrap();
    let runner = &loaded.config.local_runners["laya"];
    assert_eq!(runner.port.get(), 9000);
    assert_eq!(runner.host.as_str(), "127.0.0.1");
    assert_eq!(runner.request_timeout().as_secs(), 900);
    assert_eq!(loaded.config.workers.get(), 2);
    assert_eq!(row(&loaded, "local_runners.laya.port").value, "9000");
    assert_eq!(
        row(&loaded, "local_runners.laya.port").source,
        "DECIDR_LOCAL_RUNNERS__LAYA__PORT"
    );
    assert_eq!(
        row(&loaded, "local_runners.laya.host").source,
        global.display().to_string()
    );
    assert_eq!(row(&loaded, "workers").source, local.display().to_string());
    env(&mut o, "DECIDR_WORKERS", "3");
    o.flags.insert("workers".into(), json!(1));
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.config.workers.get(), 1);
    assert_eq!(row(&loaded, "workers").source, "--workers");
}

#[test]
fn numeric_looking_model_id_from_environment_stays_a_string() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    write(
        &global_cli_file(&o),
        &json!({"local_runners":{"fixture":{
            "kind":"generic", "host":"localhost", "port":8123, "model":"original"
        }}}),
    );
    env(&mut o, "DECIDR_LOCAL_RUNNERS__FIXTURE__MODEL", "123");

    let loaded = load_layered(&o).unwrap();

    assert_eq!(loaded.config.local_runners["fixture"].model.as_str(), "123");
    assert_eq!(row(&loaded, "local_runners.fixture.model").value, "123");
}

#[test]
fn environment_segments_named_like_fields_are_matched_by_position() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    write(
        &global_cli_file(&o),
        &json!({
            "models": {"line": {"line":"sonnet", "released":"2026-01-01"}},
            "local_runners": {"model": {"kind":"generic", "host":"localhost", "port":8123, "model":"original"}}
        }),
    );
    env(&mut o, "DECIDR_MODELS__LINE__LINE", "haiku");
    env(&mut o, "DECIDR_LOCAL_RUNNERS__MODEL__MODEL", "123");
    let loaded = load_layered(&o).unwrap();
    assert_eq!(row(&loaded, "models.line.line").value, "haiku");
    assert_eq!(
        row(&loaded, "models.line.line").source,
        "DECIDR_MODELS__LINE__LINE"
    );
    assert_eq!(loaded.config.local_runners["model"].model.as_str(), "123");
    assert_eq!(
        row(&loaded, "local_runners.model.model").source,
        "DECIDR_LOCAL_RUNNERS__MODEL__MODEL"
    );
}

#[test]
fn shared_precedence_is_local_before_global_and_cli_before_shared() {
    let root = tempfile::tempdir().unwrap();
    let o = options(root.path());
    write(&global_cli_file(&o), &json!({"project":"global-cli"}));
    write(
        &o.worktree
            .as_ref()
            .unwrap()
            .join(".sc")
            .join("sc-agent")
            .join("sc-agent.json"),
        &json!({"project":"local-shared"}),
    );
    assert_eq!(
        load_layered(&o).unwrap().config.project(),
        Some("local-shared")
    );
    write(
        &o.worktree
            .as_ref()
            .unwrap()
            .join(".sc")
            .join("sc-agent")
            .join(o.cli.file_name()),
        &json!({"project":"local-cli"}),
    );
    assert_eq!(
        load_layered(&o).unwrap().config.project(),
        Some("local-cli")
    );
}

#[test]
fn foreign_and_legacy_inputs_and_credentials_are_ignored() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    for file in ["default.json", "local.json", "sitr.json"] {
        write(&o.home.join(".sc-agent").join(file), &json!({"workers":0}));
    }
    let clean = load_layered(&o).unwrap();
    for (name, value) in [("SC_AGENT__WORKERS", "0"), ("SITR_LOG_ROOT", "elsewhere")] {
        env(&mut o, name, value);
    }
    for name in sc_agent_secrets::ALL_VARS {
        env(&mut o, name, "test-only-not-a-credential");
    }
    assert_eq!(load_layered(&o).unwrap(), clean);
    assert_eq!(doctor_rows(&load_layered(&o).unwrap()), doctor_rows(&clean));
    fs::remove_file(o.home.join(".sc-agent").join(CliName::Sitr.file_name())).unwrap();
    o.cli = CliName::Sitr;
    o.environment.clear();
    let clean = load_layered(&o).unwrap();
    env(&mut o, "DECIDR_WORKERS", "0");
    env(&mut o, "DECIDR_LOG_ROOT", "elsewhere");
    assert_eq!(load_layered(&o).unwrap(), clean);
    env(&mut o, "SITR_LOG_ROOT", "sitr-root");
    let loaded = load_layered(&o).unwrap();
    assert_eq!(row(&loaded, "log_root").value, "sitr-root");
    assert_eq!(row(&loaded, "log_root").source, "SITR_LOG_ROOT");
}

#[test]
fn canonical_shared_doctor_rows_are_identical_and_default_is_injected() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let file = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join("sc-agent.json");
    write(&file, &json!({"project":"phase-a"}));
    let expected = vec![
        DoctorRow {
            setting: "project".into(),
            value: "phase-a".into(),
            source: file.display().to_string(),
        },
        DoctorRow {
            setting: "log_root".into(),
            value: o.default_log_root.display().to_string(),
            source: "default".into(),
        },
    ];
    let decidr = load_layered(&o).unwrap();
    assert_eq!(&doctor_rows(&decidr)[..2], expected);
    o.cli = CliName::Sitr;
    let sitr = load_layered(&o).unwrap();
    assert_eq!(doctor_rows(&sitr), expected);
    assert_eq!(sitr.files.len(), 4);
    assert_eq!(sitr.files.iter().filter(|f| f.exists).count(), 1);
    o.default_log_root = root.path().join("injected-default");
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.config.log_root().unwrap(), o.default_log_root);
    assert_eq!(
        row(&loaded, "log_root").value,
        o.default_log_root.display().to_string()
    );
    assert_eq!(
        serde_json::to_value(row(&loaded, "project")).unwrap()["value"],
        "phase-a"
    );
    let cli_file = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(CliName::Sitr.file_name());
    write(&cli_file, &json!({"project":"sitr-phase"}));
    assert_eq!(
        row(&load_layered(&o).unwrap(), "project").source,
        cli_file.display().to_string()
    );
}

#[test]
fn outside_git_skips_local_files_and_loading_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let file = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(o.cli.file_name());
    write(&file, &json!({"workers":0}));
    o.worktree = None;
    let before = fs::read(&file).unwrap();
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.files.len(), 2);
    assert_eq!(loaded.config.workers.get(), 8);
    assert_eq!(fs::read(&file).unwrap(), before);
    assert!(!o.home.exists());
}

#[test]
fn scope_json_and_file_errors_name_the_source_and_keep_causes() {
    let root = tempfile::tempdir().unwrap();
    let o = options(root.path());
    let file = o.home.join(".sc-agent").join("sc-agent.json");
    write(&file, &json!({"workers":2}));
    let error = load_layered(&o).unwrap_err();
    assert!(error.message().contains("workers"));
    assert!(error.message().contains(&file.display().to_string()));
    assert_eq!(error.detail().cause.unwrap().id, "config.invalid");
    fs::write(&file, "{").unwrap();
    let error = load_layered(&o).unwrap_err();
    assert_eq!(error.id(), "config.json");
    assert!(error.message().contains(&file.display().to_string()));
    assert!(error.cause_detail().is_some());
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    assert_eq!(load_layered(&o).unwrap_err().id(), "config.not_a_file");
    fs::remove_dir(&file).unwrap();
    let f = fs::File::create(&file).unwrap();
    f.set_len(sc_agent_config::MAX_CONFIG_FILE_BYTES + 1)
        .unwrap();
    assert_eq!(load_layered(&o).unwrap_err().id(), "config.file_too_large");
}

#[test]
fn every_bound_is_checked_in_global_local_and_environment_layers() {
    let root = tempfile::tempdir().unwrap();
    let runner = json!({"kind":"laya","host":"127.0.0.1","port":8000,"model":"local/model"});
    let mut bad = vec![
        ("workers".to_string(), json!(0)),
        ("workers".into(), json!(65)),
        ("max_sends".into(), json!(0)),
        ("max_sends".into(), json!(11)),
        ("project".into(), json!("Phase B")),
    ];
    for (field, value) in [
        ("host", json!("https://host:20/path")),
        ("kind", json!("unknown")),
        ("port", json!(0)),
        ("key_env", json!("bad-key")),
        ("request_timeout_secs", json!(0)),
        ("request_timeout_secs", json!(3601)),
        ("load_timeout_secs", json!(0)),
        ("load_timeout_secs", json!(7201)),
    ] {
        let mut value_runner = runner.clone();
        value_runner[field] = value;
        bad.push((
            format!("local_runners.laya.{field}"),
            json!({"local_runners":{"laya":value_runner}}),
        ));
    }
    let mut insecure = runner.clone();
    insecure["host"] = json!("remote.example");
    insecure["key_env"] = json!("TEST_RUNNER_TOKEN");
    bad.push((
        "local_runners.laya.key_env".into(),
        json!({"local_runners":{"laya":insecure}}),
    ));
    bad.push((
        "local_runners.jev".into(),
        json!({"local_runners":{"jev":runner.clone()}}),
    ));
    for key in ["models", "local_runners"] {
        let entry = if key == "models" {
            json!({"line":"haiku","released":"2001-01-01"})
        } else {
            runner.clone()
        };
        let entries: serde_json::Map<String, Value> = (0..65)
            .map(|i| (format!("entry-{i}"), entry.clone()))
            .collect();
        bad.push((key.into(), Value::Object(entries)));
    }
    for (key, value) in bad {
        let object = if key.starts_with("local_runners.") {
            value.clone()
        } else {
            json!({key.clone():value.clone()})
        };
        for local in [false, true] {
            let o = options(root.path());
            let file = if local {
                o.worktree
                    .as_ref()
                    .unwrap()
                    .join(".sc")
                    .join("sc-agent")
                    .join(o.cli.file_name())
            } else {
                global_cli_file(&o)
            };
            write(&file, &object);
            let error = load_layered(&o).unwrap_err();
            assert!(
                error.message().contains(&file.display().to_string()),
                "{error}"
            );
            assert!(error.message().contains(&key), "{error}");
            fs::remove_file(&file).unwrap();
        }
        let mut o = options(root.path());
        let name = if key == "project" {
            "SC_PROJECT".into()
        } else if key.starts_with("local_runners.") {
            "DECIDR_LOCAL_RUNNERS".into()
        } else {
            format!("DECIDR_{}", key.to_ascii_uppercase())
        };
        let env_value = if key.starts_with("local_runners.") {
            object["local_runners"].to_string()
        } else if key == "project" {
            value.as_str().unwrap().into()
        } else {
            value.to_string()
        };
        env(&mut o, &name, &env_value);
        let error = load_layered(&o).unwrap_err();
        assert!(error.message().contains(&name), "{error}");
        assert!(error.message().contains(&key), "{error}");
    }
}

#[test]
fn invalid_lower_layer_is_not_hidden_by_a_flag() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    write(&global_cli_file(&o), &json!({"workers":0}));
    o.flags.insert("workers".into(), json!(2));
    assert!(load_layered(&o).is_err());
    fs::remove_file(global_cli_file(&o)).unwrap();
    env(&mut o, "DECIDR_WORKERS", "0");
    assert!(
        load_layered(&o)
            .unwrap_err()
            .message()
            .contains("DECIDR_WORKERS")
    );
}

#[cfg(unix)]
#[test]
fn a_non_utf8_environment_value_names_the_variable() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    o.environment.push((
        "DECIDR_WORKERS".into(),
        std::ffi::OsString::from_vec(vec![255]),
    ));
    assert!(
        load_layered(&o)
            .unwrap_err()
            .message()
            .contains("DECIDR_WORKERS")
    );
}

#[test]
fn collection_limits_and_unknown_keys_apply_to_layered_inputs() {
    for key in ["models", "local_runners"] {
        let root = tempfile::tempdir().unwrap();
        let mut o = options(root.path());
        let entry = if key == "models" {
            json!({"line":"haiku","released":"2001-01-01"})
        } else {
            json!({"kind":"laya","host":"localhost","port":8000,"model":"local/model"})
        };
        let entries: serde_json::Map<String, Value> = (0..65)
            .map(|i| (format!("entry-{i}"), entry.clone()))
            .collect();
        let name = format!("DECIDR_{}", key.to_ascii_uppercase());
        env(&mut o, &name, &Value::Object(entries).to_string());
        let error = load_layered(&o).unwrap_err();
        assert!(error.message().contains(key));
        assert!(error.message().contains(&name));
    }
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    o.flags.insert("unknown".into(), json!(2));
    assert!(
        load_layered(&o)
            .unwrap_err()
            .message()
            .contains("--unknown")
    );
    o.flags.clear();
    o.cli = CliName::Sitr;
    o.flags.insert("workers".into(), json!(2));
    assert!(load_layered(&o).is_err());
}

#[test]
fn null_log_root_is_not_allowed_to_fall_back_to_process_home() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    o.flags.insert("log_root".into(), Value::Null);
    let error = load_layered(&o).unwrap_err();
    assert!(error.message().contains("--log-root"));
    assert!(error.message().contains("log_root"));
}

#[cfg(unix)]
#[test]
fn injected_default_root_accepts_non_unicode_paths_without_serialization() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    o.default_log_root
        .push(std::ffi::OsString::from_vec(vec![255]));
    assert_eq!(
        load_layered(&o).unwrap().config.log_root().unwrap(),
        o.default_log_root
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_default_log_root_remains_a_path_and_never_panics() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    o.default_log_root = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![255]));
    let loaded = load_layered(&o).unwrap();
    assert_eq!(loaded.config.log_root().unwrap(), o.default_log_root);
    assert_eq!(row(&loaded, "log_root").source, "default");
}

#[test]
fn layered_invalid_recovery_does_not_recommend_ignored_legacy_inputs() {
    let root = tempfile::tempdir().unwrap();
    for origin in ["file", "environment", "flag"] {
        let mut o = options(root.path());
        let file = global_cli_file(&o);
        let expected_source = match origin {
            "file" => {
                write(&file, &json!({"workers":0}));
                file.display().to_string()
            }
            "environment" => {
                env(&mut o, "DECIDR_WORKERS", "0");
                "DECIDR_WORKERS".into()
            }
            "flag" => {
                o.flags.insert("workers".into(), json!(0));
                "--workers".into()
            }
            _ => unreachable!(),
        };
        let error = load_layered(&o).unwrap_err();
        assert!(error.message().contains(&expected_source));
        let advice = error.recovery().expect("actionable recovery");
        assert!(advice.contains("layer"), "{origin}: {advice}");
        assert_eq!(error.detail().recovery.as_deref(), Some(advice));
        for ignored in ["default.json", "local.json", "SC_AGENT__"] {
            assert!(!advice.contains(ignored), "{origin}: {advice}");
        }
        let AgentConfigError::Layer { cause, .. } = &error else {
            panic!("expected source context")
        };
        assert_eq!(cause.id(), "config.invalid");
        assert!(cause.message().contains("workers"));
        assert_eq!(error.cause_detail(), Some(cause.detail()));
        let detail = error.detail();
        let mut current = Some(&detail);
        let mut visited_cause = false;
        while let Some(item) = current {
            if item.id == "config.invalid" {
                visited_cause = true;
                assert!(
                    item.recovery.is_some(),
                    "{origin}: missing invalid-value recovery"
                );
            }
            if let Some(recovery) = &item.recovery {
                for ignored in ["default.json", "local.json", "SC_AGENT__"] {
                    assert!(
                        !recovery.contains(ignored),
                        "{origin}: {}: {recovery}",
                        item.id
                    );
                }
            }
            current = item.cause.as_deref();
        }
        assert!(visited_cause, "{origin}: missing invalid cause");
        if origin == "file" {
            fs::remove_file(file).unwrap();
        }
    }
}

#[test]
fn project_path_safety_is_enforced_at_each_configuration_layer() {
    let root = tempfile::tempdir().unwrap();
    let mut unsafe_names = vec![
        ".".to_owned(),
        "..".to_owned(),
        "phase.".to_owned(),
        "con".to_owned(),
        "nul".to_owned(),
        "aux".to_owned(),
        "com1".to_owned(),
        "lpt9".to_owned(),
        "con.txt".to_owned(),
    ];
    unsafe_names.push("a".repeat(MAX_SEGMENT_BYTES + 1));
    for name in unsafe_names {
        for origin in ["file", "environment", "flag"] {
            let mut o = options(root.path());
            let file = global_cli_file(&o);
            let expected_source = match origin {
                "file" => {
                    write(&file, &json!({"project":name}));
                    file.display().to_string()
                }
                "environment" => {
                    env(&mut o, "SC_PROJECT", &name);
                    "SC_PROJECT".into()
                }
                "flag" => {
                    o.flags.insert("project".into(), json!(name));
                    "--project".into()
                }
                _ => unreachable!(),
            };
            let error =
                load_layered(&o).expect_err(&format!("{origin} accepted unsafe project {name:?}"));
            assert!(error.message().contains("project"), "{error}");
            assert!(error.message().contains(&expected_source), "{error}");
            if origin == "file" {
                fs::remove_file(file).unwrap();
            }
        }
    }
}

#[test]
fn valid_project_slugs_are_returned_unchanged_from_every_input_kind() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "a.b_c-1".to_owned(),
        "console".to_owned(),
        "com10".to_owned(),
        "a".repeat(MAX_SEGMENT_BYTES),
    ] {
        for origin in ["file", "environment", "flag"] {
            let mut o = options(root.path());
            let file = global_cli_file(&o);
            match origin {
                "file" => write(&file, &json!({"project":name})),
                "environment" => env(&mut o, "SC_PROJECT", &name),
                "flag" => {
                    o.flags.insert("project".into(), json!(name));
                }
                _ => unreachable!(),
            }
            let loaded = load_layered(&o).unwrap();
            assert_eq!(loaded.config.project(), Some(name.as_str()));
            assert_eq!(loaded.config.require_project().unwrap(), name);
            if origin == "file" {
                fs::remove_file(file).unwrap();
            }
        }
    }
}

#[test]
fn validation_names_the_winning_key_source_despite_unrelated_lower_files() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let file = global_cli_file(&o);
    write(&file, &json!({"workers": 2}));
    o.flags.insert("project".into(), json!("Phase B"));
    let AgentConfigError::Layer { source, cause } = load_layered(&o).unwrap_err() else {
        panic!("layer");
    };
    assert_eq!(source, "--project");
    assert!(matches!(*cause, AgentConfigError::Invalid { ref path, .. } if path == "project"));
    o.flags.clear();
    env(&mut o, "DECIDR_WORKERS", "0");
    let AgentConfigError::Layer { source, cause } = load_layered(&o).unwrap_err() else {
        panic!("layer");
    };
    assert_eq!(source, "DECIDR_WORKERS");
    assert!(matches!(*cause, AgentConfigError::Invalid { ref path, .. } if path == "workers"));
    // Masked invalid input is still rejected and keeps its own source.
    o.environment.clear();
    write(&file, &json!({"workers": 0}));
    o.flags.insert("workers".into(), json!(3));
    let AgentConfigError::Layer { source, cause } = load_layered(&o).unwrap_err() else {
        panic!("layer");
    };
    assert_eq!(source, file.display().to_string());
    assert!(matches!(*cause, AgentConfigError::Invalid { ref path, .. } if path == "workers"));
}

fn failure_source(o: &LoadOptions, expected_source: &str, expected_path: &str) {
    let AgentConfigError::Layer { source, cause } = load_layered(o).expect_err("invalid layer")
    else {
        panic!("source-qualified failure");
    };
    assert_eq!(source, expected_source);
    let path = match *cause {
        AgentConfigError::Invalid { path, .. }
        | AgentConfigError::RunnerName { path, .. }
        | AgentConfigError::KeyOnPlainHttp { path, .. } => path,
        other => panic!("unexpected cause: {other:?}"),
    };
    assert_eq!(path, expected_path);
}

#[test]
fn nested_port_failures_name_flag_env_and_file_over_unrelated_lower_values() {
    for source in ["flag", "env", "file"] {
        let root = tempfile::tempdir().unwrap();
        let mut o = options(root.path());
        let global = global_cli_file(&o);
        write(
            &global,
            &json!({"workers": 2, "local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"m"}}}),
        );
        let local = o
            .worktree
            .as_ref()
            .unwrap()
            .join(".sc")
            .join("sc-agent")
            .join(o.cli.file_name());
        let origin = match source {
            "flag" => {
                o.flags.insert("local_runners.laya.port".into(), json!(0));
                "--local-runners.laya.port".into()
            }
            "env" => {
                env(&mut o, "DECIDR_LOCAL_RUNNERS__LAYA__PORT", "0");
                "DECIDR_LOCAL_RUNNERS__LAYA__PORT".into()
            }
            _ => {
                write(&local, &json!({"local_runners":{"laya":{"port":0}}}));
                local.display().to_string()
            }
        };
        failure_source(&o, &origin, "local_runners.laya.port");
    }
}

#[test]
fn parent_runner_errors_name_the_highest_precedence_descendant() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let global = global_cli_file(&o);
    let local = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(o.cli.file_name());
    write(
        &global,
        &json!({"workers":2,"local_runners":{"laya":{"port":1}}}),
    );
    write(
        &local,
        &json!({"local_runners":{"laya":{"host":"localhost"}}}),
    );
    failure_source(&o, &local.display().to_string(), "local_runners.laya");
    env(&mut o, "DECIDR_LOCAL_RUNNERS__LAYA__PORT", "2");
    failure_source(&o, "DECIDR_LOCAL_RUNNERS__LAYA__PORT", "local_runners.laya");
    o.flags.insert("local_runners.laya.port".into(), json!(3));
    failure_source(&o, "--local-runners.laya.port", "local_runners.laya");
}

#[test]
fn short_name_runner_key_and_split_plain_http_key_keep_their_sources() {
    let root = tempfile::tempdir().unwrap();
    let o = options(root.path());
    let global = global_cli_file(&o);
    let local = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(o.cli.file_name());
    write(&global, &json!({"workers":2}));
    write(
        &local,
        &json!({"local_runners":{"haiku":{"kind":"laya","host":"localhost","port":1,"model":"m"}}}),
    );
    failure_source(&o, &local.display().to_string(), "local_runners.haiku");
    write(
        &global,
        &json!({"workers":2,"local_runners":{"laya":{"kind":"laya","host":"remote.example","port":1,"model":"m"}}}),
    );
    write(
        &local,
        &json!({"local_runners":{"laya":{"key_env":sc_agent_secrets::LAYA_API_KEY}}}),
    );
    failure_source(
        &o,
        &local.display().to_string(),
        "local_runners.laya.key_env",
    );
}

#[test]
fn model_entry_and_parent_count_errors_keep_their_sources() {
    let root = tempfile::tempdir().unwrap();
    let o = options(root.path());
    let global = global_cli_file(&o);
    let local = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(o.cli.file_name());
    write(&global, &json!({"workers":2}));
    write(
        &local,
        &json!({"models":{"custom":{"line":"haiku","released":"not-a-date"}}}),
    );
    failure_source(&o, &local.display().to_string(), "models.custom.released");
    let models: serde_json::Map<String, Value> = (0..=sc_agent_config::MAX_MODELS)
        .map(|i| {
            (
                format!("custom-{i}"),
                json!({"line":"haiku","released":"2026-01-01"}),
            )
        })
        .collect();
    write(&local, &json!({"models":models}));
    failure_source(&o, &local.display().to_string(), "models");
}

#[test]
fn exact_child_provenance_wins_over_its_recorded_parent() {
    let root = tempfile::tempdir().unwrap();
    let mut o = options(root.path());
    let global = global_cli_file(&o);
    write(&global, &json!({"local_runners":{}}));
    o.flags.insert(
        "local_runners.laya".into(),
        json!({"kind":"laya","host":"localhost","port":0,"model":"m"}),
    );
    failure_source(&o, "--local-runners.laya", "local_runners.laya.port");
}

#[test]
fn model_count_limit_uses_the_source_of_the_parent_descendants() {
    let root = tempfile::tempdir().unwrap();
    let o = options(root.path());
    let global = global_cli_file(&o);
    let local = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc")
        .join("sc-agent")
        .join(o.cli.file_name());
    write(&global, &json!({"workers":2}));
    let models: serde_json::Map<String, Value> = (0..=sc_agent_config::MAX_MODELS)
        .map(|i| {
            (
                format!("custom-{i}"),
                json!({"line":"haiku","released":"2026-01-01"}),
            )
        })
        .collect();
    write(&local, &json!({"models":models}));
    failure_source(&o, &local.display().to_string(), "models");
}
