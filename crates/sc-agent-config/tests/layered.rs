//! The layered loader only uses temporary files and explicitly supplied environment values.
#![cfg(test)]
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use sc_agent_config::{
    AgentConfigError, CliName, DoctorRow, LoadOptions, Source, doctor_rows, load_layered,
};
use sc_agent_types::{ErrorCategory, ErrorId};
use serde_json::{Value, json};

fn options(root: &Path) -> LoadOptions {
    LoadOptions {
        cli: CliName::Decidr,
        home: root.join("home"),
        worktree: Some(root.join("work")),
        default_log_root: root.join("home/.sc-agent"),
        flags: BTreeMap::new(),
        environment: Vec::new(),
    }
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
        o.home.join(".sc-agent/sc-agent.json"),
        o.home.join(".sc-agent/decidr.json"),
        o.worktree
            .as_ref()
            .unwrap()
            .join(".sc/sc-agent/sc-agent.json"),
        o.worktree
            .as_ref()
            .unwrap()
            .join(".sc/sc-agent/decidr.json"),
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
    let global = o.home.join(".sc-agent/decidr.json");
    let local = o
        .worktree
        .as_ref()
        .unwrap()
        .join(".sc/sc-agent/decidr.json");
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
fn shared_precedence_is_local_before_global_and_cli_before_shared() {
    let root = tempfile::tempdir().unwrap();
    let o = options(root.path());
    write(
        &o.home.join(".sc-agent/decidr.json"),
        &json!({"project":"global-cli"}),
    );
    write(
        &o.worktree
            .as_ref()
            .unwrap()
            .join(".sc/sc-agent/sc-agent.json"),
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
            .join(".sc/sc-agent/decidr.json"),
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
    fs::remove_file(o.home.join(".sc-agent/sitr.json")).unwrap();
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
        .join(".sc/sc-agent/sc-agent.json");
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
    let cli_file = o.worktree.as_ref().unwrap().join(".sc/sc-agent/sitr.json");
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
        .join(".sc/sc-agent/decidr.json");
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
    let file = o.home.join(".sc-agent/sc-agent.json");
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
                    .join(".sc/sc-agent/decidr.json")
            } else {
                o.home.join(".sc-agent/decidr.json")
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
    write(&o.home.join(".sc-agent/decidr.json"), &json!({"workers":0}));
    o.flags.insert("workers".into(), json!(2));
    assert!(load_layered(&o).is_err());
    fs::remove_file(o.home.join(".sc-agent/decidr.json")).unwrap();
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
        let file = o.home.join(".sc-agent/decidr.json");
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
        let AgentConfigError::Layer { cause, .. } = &error else {
            panic!("expected a layered error")
        };
        assert_eq!(cause.id(), "config.invalid");
        assert_eq!(error.detail().cause.as_deref(), Some(&cause.detail()));
        for ignored in ["default.json", "local.json", "SC_AGENT__"] {
            assert!(!advice.contains(ignored), "{origin}: {advice}");
        }
        assert!(advice.contains("layer"), "{origin}: {advice}");
        assert_eq!(error.detail().recovery.as_deref(), Some(advice));
        let AgentConfigError::Layer { cause, .. } = &error else {
            panic!("expected source context")
        };
        assert_eq!(cause.id(), "config.invalid");
        assert!(cause.message().contains("workers"));
        assert_eq!(error.cause_detail(), Some(cause.detail()));
        if origin == "file" {
            fs::remove_file(file).unwrap();
        }
    }
}
