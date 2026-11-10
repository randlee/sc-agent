//! Loading the global config (REQ-SAG-0102, 0601, 0701, 0801, 0003, 0002, 0013, 0015; ADR-SAG-0002).
//!
//! Every test writes its files under a temp dir and passes an explicit
//! environment `Vec`: the process environment and HOME are never changed.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use sc_agent_config::{
    AgentConfig, AgentConfigError, DEFAULT_MAX_SENDS, DEFAULT_RUNNER_LOAD_TIMEOUT,
    DEFAULT_RUNNER_REQUEST_TIMEOUT, DEFAULT_WORKERS, MAX_CONFIG_FILE_BYTES, MAX_LOCAL_RUNNERS,
    MAX_MAX_SENDS, MAX_MODELS, MAX_WORKERS, RunnerKind, RunnerName, default_dir, load,
};
use sc_agent_types::{ErrorId, ModelChoice, ShortName};

type Env = Vec<(OsString, OsString)>;

fn env(pairs: &[(&str, &str)]) -> Env {
    pairs
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect()
}

/// Fixtures that unwrap or panic on a broken temp dir or an unexpected result.
///
/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap and panic); these helpers are test
/// code, so they live in a `#[cfg(test)]` module.
#[cfg(test)]
mod support {
    use std::fs;

    use sc_agent_config::{AgentConfig, AgentConfigError, load};
    use sc_agent_types::ErrorId;

    pub(crate) fn dir_with(default_json: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("default.json"), default_json).unwrap();
        dir
    }

    pub(crate) fn load_json(default_json: &str) -> Result<AgentConfig, AgentConfigError> {
        let dir = dir_with(default_json);
        load(dir.path().to_path_buf(), Vec::new())
    }

    /// Asserts `json` fails as config.invalid at `path`.
    pub(crate) fn assert_invalid(json: &str, path: &str) -> String {
        match load_json(json) {
            Err(AgentConfigError::Invalid { path: got, message }) => {
                assert_eq!(got, path, "{json}: {message}");
                let error = AgentConfigError::Invalid {
                    path: got,
                    message: message.clone(),
                };
                assert_eq!(error.id(), "config.invalid");
                assert!(error.to_string().contains(path));
                message
            }
            other => panic!("{json}: expected config.invalid at {path}, got {other:?}"),
        }
    }
}

use support::{assert_invalid, dir_with, load_json};

/// A runner entry with `fields` added to the minimal valid set.
fn runner(fields: &str) -> String {
    let base =
        r#""kind": "laya", "host": "127.0.0.1", "port": 8000, "model": "convaiinnovations/laya""#;
    if fields.is_empty() {
        format!("{{ {base} }}")
    } else {
        format!("{{ {base}, {fields} }}")
    }
}

/// A runner entry built from exactly `fields`.
fn runner_only(fields: &str) -> String {
    format!(r#"{{ "local_runners": {{ "laya": {{ {fields} }} }} }}"#)
}

const EXAMPLE: &str = r#"{ "workers": 8, "max_sends": 5,
  "models": { "claude-haiku-5-6": { "line": "haiku", "released": "2026-12-01" } },
  "local_runners": { "laya": { "kind": "laya", "host": "127.0.0.1", "port": 8000,
    "model": "convaiinnovations/laya", "key_env": "MY_RUNNER_TOKEN" } } }"#;

/// AC REQ-SAG-0102 AC1/AC2: a later-dated haiku becomes latest; an earlier one does not.
#[test]
fn a_configured_model_dated_later_becomes_the_latest_of_its_line() {
    let builtin =
        sc_agent_types::Catalog::builtin().resolve(&ModelChoice::Latest(ShortName::Haiku));

    let later = load_json(
        r#"{ "models": { "claude-haiku-9-9": { "line": "haiku", "released": "2099-01-01" } } }"#,
    )
    .unwrap();
    let catalog = later.catalog().unwrap();
    assert_eq!(
        catalog
            .resolve(&ModelChoice::Latest(ShortName::Haiku))
            .as_str(),
        "claude-haiku-9-9"
    );
    assert_eq!(catalog.line_of("claude-haiku-9-9"), Some(ShortName::Haiku));

    let earlier = load_json(
        r#"{ "models": { "claude-haiku-0-1": { "line": "haiku", "released": "2001-01-01" } } }"#,
    )
    .unwrap();
    let catalog = earlier.catalog().unwrap();
    assert_eq!(
        catalog.resolve(&ModelChoice::Latest(ShortName::Haiku)),
        builtin
    );
    assert_eq!(catalog.line_of("claude-haiku-0-1"), Some(ShortName::Haiku));
}

/// AC REQ-SAG-0102: every key loads from JSON, and SC_AGENT__ overrides apply.
#[test]
fn every_key_loads_and_env_overrides_apply() {
    let dir = dir_with(EXAMPLE);
    let config = load(dir.path().to_path_buf(), Vec::new()).unwrap();
    assert_eq!(config.workers.get(), 8);
    assert_eq!(config.max_sends.get(), 5);
    assert_eq!(config.log_root, None);
    assert_eq!(config.models.len(), 1);
    let (id, extra) = config.models.iter().next().unwrap();
    assert_eq!(id.as_str(), "claude-haiku-5-6");
    assert_eq!(extra.line, ShortName::Haiku);
    assert_eq!(extra.released.to_string(), "2026-12-01");
    let laya = config.local_runners.get("laya").unwrap();
    assert_eq!(laya.port.get(), 8000);

    let overridden = load(
        dir.path().to_path_buf(),
        env(&[
            ("SC_AGENT__MAX_SENDS", "2"),
            ("SC_AGENT__LOCAL_RUNNERS__LAYA__PORT", "9000"),
            ("SC_AGENT__LOG_ROOT", "/var/tmp/sc-agent-logs"),
            ("UNRELATED", "1"),
        ]),
    )
    .unwrap();
    assert_eq!(overridden.max_sends.get(), 2);
    assert_eq!(
        overridden.local_runners.get("laya").unwrap().port.get(),
        9000
    );
    assert_eq!(
        overridden.log_root,
        Some(PathBuf::from("/var/tmp/sc-agent-logs"))
    );
}

/// local.json is merged over default.json.
#[test]
fn local_json_overrides_default_json() {
    let dir = dir_with(EXAMPLE);
    fs::write(
        dir.path().join("local.json"),
        r#"{ "workers": 3, "local_runners": { "laya": { "port": 8100 } } }"#,
    )
    .unwrap();
    let config = load(dir.path().to_path_buf(), Vec::new()).unwrap();
    assert_eq!(config.workers.get(), 3);
    let laya = config.local_runners.get("laya").unwrap();
    assert_eq!(laya.port.get(), 8100);
    assert_eq!(laya.host.as_str(), "127.0.0.1");
}

/// AC REQ-SAG-0601: max_sends default 5; 2 via file or env; 0 and 11 rejected.
#[test]
fn max_sends_default_override_and_bounds() {
    assert_eq!(load_json("{}").unwrap().max_sends, DEFAULT_MAX_SENDS);
    assert_eq!(DEFAULT_MAX_SENDS.get(), 5);
    assert_eq!(
        load_json(r#"{ "max_sends": 2 }"#).unwrap().max_sends.get(),
        2
    );
    let dir = dir_with("{}");
    let from_env = load(
        dir.path().to_path_buf(),
        env(&[("SC_AGENT__MAX_SENDS", "2")]),
    )
    .unwrap();
    assert_eq!(from_env.max_sends.get(), 2);
    assert_eq!(MAX_MAX_SENDS, 10);
    assert_eq!(
        load_json(r#"{ "max_sends": 1 }"#).unwrap().max_sends.get(),
        1
    );
    assert_eq!(
        load_json(r#"{ "max_sends": 10 }"#).unwrap().max_sends.get(),
        10
    );
    for bad in ["0", "11", "-1", "\"five\""] {
        assert_invalid(&format!(r#"{{ "max_sends": {bad} }}"#), "max_sends");
    }
}

/// AC REQ-SAG-0701 / REQ-SAG-0013: workers default 8; 0 and 65 rejected; 1 and 64 load.
#[test]
fn workers_default_and_bounds() {
    assert_eq!(load_json("{}").unwrap().workers, DEFAULT_WORKERS);
    assert_eq!(DEFAULT_WORKERS.get(), 8);
    assert_eq!(MAX_WORKERS, 64);
    assert_eq!(load_json(r#"{ "workers": 1 }"#).unwrap().workers.get(), 1);
    assert_eq!(load_json(r#"{ "workers": 64 }"#).unwrap().workers.get(), 64);
    for bad in ["0", "65"] {
        assert_invalid(&format!(r#"{{ "workers": {bad} }}"#), "workers");
    }
}

/// AC REQ-SAG-0801 (config part): each runner carries name, kind, host, port, model, key_env.
#[test]
fn local_runners_load_by_name_from_the_file() {
    let json = format!(
        r#"{{ "local_runners": {{
            "laya": {},
            "box": {{ "kind": "strands", "host": "runner-box.lan", "port": 7000, "model": "some/model" }},
            "mapi": {{ "kind": "mapika", "host": "::1", "port": 7100, "model": "m" }},
            "gen": {{ "kind": "generic", "host": "LocalHost", "port": 7200, "model": "g", "key_env": "MY_RUNNER_TOKEN" }}
        }} }}"#,
        runner(r#""key_env": "MY_RUNNER_TOKEN""#)
    );
    let config = load_json(&json).unwrap();
    let names: Vec<&str> = config
        .local_runners
        .keys()
        .map(RunnerName::as_str)
        .collect();
    assert_eq!(names, ["box", "gen", "laya", "mapi"]);

    let laya = config.local_runners.get("laya").unwrap();
    assert_eq!(laya.kind, RunnerKind::Laya);
    assert_eq!(laya.host.as_str(), "127.0.0.1");
    assert!(laya.host.is_loopback());
    assert_eq!(laya.port.get(), 8000);
    assert_eq!(laya.model.as_str(), "convaiinnovations/laya");
    assert_eq!(
        laya.key_env.as_ref().map(sc_agent_types::VarName::as_str),
        Some("MY_RUNNER_TOKEN")
    );

    let other = config.local_runners.get("box").unwrap();
    assert_eq!(other.kind, RunnerKind::Strands);
    assert_eq!(other.host.as_ref(), "runner-box.lan");
    assert!(!other.host.is_loopback());
    assert_eq!(other.key_env, None);
    assert_eq!(
        config.local_runners.get("mapi").unwrap().kind,
        RunnerKind::Mapika
    );
    assert_eq!(
        config.local_runners.get("gen").unwrap().kind,
        RunnerKind::Generic
    );
}

/// AC REQ-SAG-0003 and ADR-SAG-0002: log_root is the configured value, else ~/.sc-agent.
#[test]
fn log_root_is_configured_value_else_default_dir() {
    let configured = load_json(r#"{ "log_root": "/srv/sc-agent-logs" }"#).unwrap();
    assert_eq!(
        configured.log_root().unwrap(),
        PathBuf::from("/srv/sc-agent-logs")
    );

    let default = load_json("{}").unwrap();
    // Read-only: HOME is never changed by these tests.
    let home = std::env::home_dir().unwrap();
    assert_eq!(default_dir().unwrap(), home.join(".sc-agent"));
    assert_eq!(default.log_root().unwrap(), default_dir().unwrap());
    assert_eq!(
        AgentConfig::default().log_root().unwrap(),
        home.join(".sc-agent")
    );
}

/// AC: a missing default.json is AgentConfig::default() unless an override is set.
#[test]
fn missing_default_json_is_default_unless_overridden() {
    let empty = tempfile::tempdir().unwrap();
    let config = load(
        empty.path().to_path_buf(),
        env(&[("OTHER_APP__WORKERS", "3")]),
    )
    .unwrap();
    assert_eq!(config, AgentConfig::default());

    let error = load(
        empty.path().to_path_buf(),
        env(&[("SC_AGENT__WORKERS", "3")]),
    )
    .unwrap_err();
    assert_eq!(error.id(), "config.load");
    assert_eq!(error.cause_detail().unwrap().id, "CFG.MISSING_DEFAULT");

    // A local.json with no default.json is not ignored either.
    fs::write(empty.path().join("local.json"), r#"{ "workers": 3 }"#).unwrap();
    let error = load(empty.path().to_path_buf(), Vec::new()).unwrap_err();
    assert_eq!(error.id(), "config.load");
    assert_eq!(error.cause_detail().unwrap().id, "CFG.MISSING_DEFAULT");
}

/// AC: a local runner named like a short name is config.runner_name.
#[test]
fn a_runner_named_like_a_short_name_is_rejected() {
    for name in ["haiku", "jev", "Astra"] {
        let json = format!(r#"{{ "local_runners": {{ "{name}": {} }} }}"#, runner(""));
        match load_json(&json).unwrap_err() {
            AgentConfigError::RunnerName { name: got, path } => {
                assert_eq!(got, name);
                assert_eq!(path, format!("local_runners.{name}"));
            }
            other => panic!("expected config.runner_name, got {other:?}"),
        }
    }
}

/// AC REQ-SAG-0002 (config half): ids, Display, causes for bad JSON and a model on two lines.
#[test]
fn config_errors_follow_the_error_contract() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("default.json"), "{ \"workers\": ").unwrap();
    let error = load(dir.path().to_path_buf(), Vec::new()).unwrap_err();
    assert_eq!(error.id(), "config.load");
    assert_display_is_id_and_message(&error);
    let cause = error.cause_detail().unwrap();
    assert_eq!(cause.id, "CFG.PARSE");
    let file = dir.path().join("default.json");
    assert!(
        cause.message.contains(&file.display().to_string()),
        "{}",
        cause.message
    );
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(error.detail().cause.as_deref(), Some(&cause));
    let recovery = error.recovery().unwrap();
    assert!(
        recovery.contains("JSON"),
        "sc-config's advice for CFG.PARSE: {recovery}"
    );

    // claude-haiku-5-5 is a built-in haiku; putting it on the sonnet line conflicts.
    let config = load_json(
        r#"{ "models": { "claude-haiku-5-5": { "line": "sonnet", "released": "2026-12-01" } } }"#,
    )
    .unwrap();
    let error = config.catalog().unwrap_err();
    assert_eq!(error.id(), "config.catalog");
    assert_display_is_id_and_message(&error);
    assert_eq!(error.cause_detail().unwrap().id, "types.model_on_two_lines");
    assert!(std::error::Error::source(&error).is_some());
    let recovery = error.recovery().unwrap();
    assert!(recovery.contains("different id"), "{recovery}");

    let others = [
        AgentConfigError::HomeUnknown,
        AgentConfigError::FileTooLarge {
            file: PathBuf::from("/cfg/default.json"),
            bytes: 2_000_000,
        },
        AgentConfigError::RunnerName {
            name: "haiku".to_owned(),
            path: "local_runners.haiku".to_owned(),
        },
        AgentConfigError::Invalid {
            path: "workers".to_owned(),
            message: "workers must be 1..=64, got 0".to_owned(),
        },
        AgentConfigError::KeyOnPlainHttp {
            path: "local_runners.laya.key_env".to_owned(),
            host: "10.0.0.5".to_owned(),
        },
    ];
    let ids: Vec<&str> = others.iter().map(ErrorId::id).collect();
    assert_eq!(
        ids,
        [
            "config.home_unknown",
            "config.file_too_large",
            "config.runner_name",
            "config.invalid",
            "config.key_on_plain_http",
        ]
    );
    for error in &others {
        assert_display_is_id_and_message(error);
        assert!(error.cause_detail().is_none());
        assert!(std::error::Error::source(error).is_none());
    }
    // Every variant says what to do next, and the advice fits the failure.
    let expected = [
        ["HOME", "log_root", "config_dir"],
        ["1 MiB", "shrink", "config file"],
        ["rename", "runner", "rename the runner"],
        ["fix the value", "SC_AGENT__", "local.json"],
        ["loopback", "key_env", "remove"],
    ];
    for (error, words) in others.iter().zip(expected) {
        let recovery = error.recovery().unwrap_or_default();
        for word in words {
            assert!(recovery.contains(word), "{}: {recovery}", error.id());
        }
        assert_eq!(error.detail().recovery.as_deref(), Some(recovery));
    }
}

/// REQ-SAG-0013: at most 64 models and 64 runners; one more is config.invalid at the map.
#[test]
fn models_and_runners_are_capped_at_64_entries() {
    assert_eq!((MAX_MODELS, MAX_LOCAL_RUNNERS), (64, 64));
    let models = |count: usize| {
        let entries: Vec<String> = (0..count)
            .map(|index| {
                format!(r#""extra-{index}": {{ "line": "haiku", "released": "2001-01-01" }}"#)
            })
            .collect();
        format!(r#"{{ "models": {{ {} }} }}"#, entries.join(", "))
    };
    assert_eq!(load_json(&models(64)).unwrap().models.len(), 64);
    let message = assert_invalid(&models(65), "models");
    assert!(message.contains("at most 64"), "{message}");

    let runners = |count: usize| {
        let entries: Vec<String> = (0..count)
            .map(|index| format!(r#""runner-{index}": {}"#, runner("")))
            .collect();
        format!(r#"{{ "local_runners": {{ {} }} }}"#, entries.join(", "))
    };
    assert_eq!(load_json(&runners(64)).unwrap().local_runners.len(), 64);
    let message = assert_invalid(&runners(65), "local_runners");
    assert!(message.contains("at most 64"), "{message}");
}

/// A config file over 1 MiB is refused before sc-config reads it; exactly 1 MiB loads.
#[test]
fn an_oversized_config_file_is_refused_before_reading() {
    let limit = usize::try_from(MAX_CONFIG_FILE_BYTES).unwrap();
    let padded = |len: usize| format!("{{}}{}", " ".repeat(len - 2));
    for file in ["default.json", "local.json"] {
        let dir = dir_with("{}");
        fs::write(dir.path().join(file), padded(limit + 1)).unwrap();
        let error = load(dir.path().to_path_buf(), Vec::new()).unwrap_err();
        assert_eq!(error.id(), "config.file_too_large", "{file}");
        let AgentConfigError::FileTooLarge { file: got, bytes } = &error else {
            unreachable!("checked by id")
        };
        assert_eq!(got, &dir.path().join(file));
        assert_eq!(*bytes, MAX_CONFIG_FILE_BYTES + 1);

        fs::write(dir.path().join(file), padded(limit)).unwrap();
        assert_eq!(
            load(dir.path().to_path_buf(), Vec::new()).unwrap(),
            AgentConfig::default()
        );
    }
}

/// RunnerName refuses an empty name and a short name; other names round-trip.
#[test]
fn runner_name_validates_at_deserialization() {
    assert_eq!(
        RunnerName::try_from("laya".to_owned()).unwrap().as_str(),
        "laya"
    );
    assert!(RunnerName::try_from("Sonnet".to_owned()).is_err());
    assert!(RunnerName::try_from(String::new()).is_err());
    let json = format!(r#"{{ "local_runners": {{ "": {} }} }}"#, runner(""));
    let message = assert_invalid(&json, "local_runners.");
    assert!(message.contains("must not be empty"), "{message}");
}

fn assert_display_is_id_and_message(error: &AgentConfigError) {
    assert_eq!(
        error.to_string(),
        format!("{}: {}", error.id(), error.message())
    );
}

/// An unknown key is config.invalid naming it.
#[test]
fn an_unknown_key_is_invalid_with_its_path() {
    let message = assert_invalid(r#"{ "wrokers": 3 }"#, "wrokers");
    assert!(message.contains("wrokers"), "{message}");
    let json = format!(
        r#"{{ "local_runners": {{ "laya": {} }} }}"#,
        runner(r#""prot": 1"#)
    );
    assert_invalid(&json, "local_runners.laya.prot");
}

/// AC REQ-SAG-0013: every bound fails at load as config.invalid with the key path.
#[test]
fn every_value_bound_is_checked_at_load_with_its_path() {
    // Model rows: empty id, unknown short name, bad date.
    assert_invalid(
        r#"{ "models": { "claude-x": { "line": "nope", "released": "2026-12-01" } } }"#,
        "models.claude-x.line",
    );
    assert_invalid(
        r#"{ "models": { "claude-x": { "line": "haiku", "released": "2026-13-01" } } }"#,
        "models.claude-x.released",
    );
    let message = assert_invalid(
        r#"{ "models": { "": { "line": "haiku", "released": "2026-12-01" } } }"#,
        // The empty key itself ends the path.
        "models.",
    );
    assert!(message.contains("types.invalid_model_id"), "{message}");

    // Runner fields.
    for host in [
        "127.0.0.1/x",
        "user@host",
        "host:8000",
        "a host",
        "",
        "-bad.lan",
        "bad-.lan",
        "a..b",
    ] {
        let json = runner_only(&format!(
            r#""kind": "laya", "host": "{host}", "port": 8000, "model": "m""#
        ));
        let message = assert_invalid(&json, "local_runners.laya.host");
        assert!(message.contains("hostname or IP address"), "{message}");
    }
    let long_label = "a".repeat(64);
    assert_invalid(
        &runner_only(&format!(
            r#""kind": "laya", "host": "{long_label}.lan", "port": 8000, "model": "m""#
        )),
        "local_runners.laya.host",
    );
    assert_invalid(
        &runner_only(r#""kind": "ollama", "host": "127.0.0.1", "port": 8000, "model": "m""#),
        "local_runners.laya.kind",
    );
    for port in ["0", "65536", "-1"] {
        assert_invalid(
            &runner_only(&format!(
                r#""kind": "laya", "host": "127.0.0.1", "port": {port}, "model": "m""#
            )),
            "local_runners.laya.port",
        );
    }
    assert_invalid(
        &runner_only(r#""kind": "laya", "host": "127.0.0.1", "port": 8000, "model": """#),
        "local_runners.laya.model",
    );
    for key_env in ["lower-case", "1X", "", "A-B"] {
        let message = assert_invalid(
            &runner_only(&format!(
                r#""kind": "laya", "host": "127.0.0.1", "port": 8000, "model": "m", "key_env": "{key_env}""#
            )),
            "local_runners.laya.key_env",
        );
        assert!(message.contains("types.invalid_var_name"), "{message}");
    }
    // A missing required field is reported at its runner.
    assert_invalid(
        &runner_only(r#""kind": "laya", "host": "127.0.0.1", "port": 8000"#),
        "local_runners.laya",
    );
}

/// AC REQ-SAG-0013: key_env on a non-loopback host is config.key_on_plain_http; loopback loads.
#[test]
fn key_env_requires_a_loopback_host() {
    for host in [
        "10.0.0.5",
        "example.com",
        "128.0.0.1",
        "::2",
        "localhost.example.com",
    ] {
        let json = runner_only(&format!(
            r#""kind": "laya", "host": "{host}", "port": 8000, "model": "m", "key_env": "MY_RUNNER_TOKEN""#
        ));
        match load_json(&json).unwrap_err() {
            error @ AgentConfigError::KeyOnPlainHttp { .. } => {
                assert_eq!(error.id(), "config.key_on_plain_http");
                let AgentConfigError::KeyOnPlainHttp { path, host: got } = &error else {
                    unreachable!()
                };
                assert_eq!(path, "local_runners.laya.key_env");
                assert_eq!(got, host);
            }
            other => panic!("{host}: expected config.key_on_plain_http, got {other:?}"),
        }
        // The same runner without key_env loads.
        let plain = runner_only(&format!(
            r#""kind": "laya", "host": "{host}", "port": 8000, "model": "m""#
        ));
        load_json(&plain).unwrap();
    }
    for host in ["127.0.0.1", "127.9.8.7", "::1", "localhost", "LOCALHOST"] {
        let json = runner_only(&format!(
            r#""kind": "laya", "host": "{host}", "port": 8000, "model": "m", "key_env": "MY_RUNNER_TOKEN""#
        ));
        let config = load_json(&json).unwrap();
        assert!(config.local_runners.get("laya").unwrap().host.is_loopback());
    }
}

/// AC REQ-SAG-0015: an entry without timeouts gives 300 s and 600 s.
#[test]
fn runner_timeouts_default_to_300_and_600_seconds() {
    let config = load_json(&format!(
        r#"{{ "local_runners": {{ "laya": {} }} }}"#,
        runner("")
    ))
    .unwrap();
    let laya = config.local_runners.get("laya").unwrap();
    assert_eq!(laya.request_timeout(), Duration::from_secs(300));
    assert_eq!(laya.load_timeout(), Duration::from_secs(600));
    assert_eq!(DEFAULT_RUNNER_REQUEST_TIMEOUT, Duration::from_secs(300));
    assert_eq!(DEFAULT_RUNNER_LOAD_TIMEOUT, Duration::from_secs(600));
}

/// AC REQ-SAG-0015: timeouts set on one runner change only that runner.
#[test]
fn runner_timeouts_apply_per_runner() {
    let json = format!(
        r#"{{ "local_runners": {{ "laya": {}, "other": {} }} }}"#,
        runner(r#""request_timeout_secs": 45, "load_timeout_secs": 1200"#),
        runner("")
    );
    let config = load_json(&json).unwrap();
    let laya = config.local_runners.get("laya").unwrap();
    assert_eq!(laya.request_timeout(), Duration::from_secs(45));
    assert_eq!(laya.load_timeout(), Duration::from_secs(1200));
    let other = config.local_runners.get("other").unwrap();
    assert_eq!(other.request_timeout(), Duration::from_secs(300));
    assert_eq!(other.load_timeout(), Duration::from_secs(600));
}

/// AC REQ-SAG-0015: out-of-range timeouts are config.invalid; the bounds themselves load.
#[test]
fn runner_timeout_bounds() {
    let at = |fields: &str| format!(r#"{{ "local_runners": {{ "laya": {} }} }}"#, runner(fields));
    for bad in [9, 3601, 0] {
        assert_invalid(
            &at(&format!(r#""request_timeout_secs": {bad}"#)),
            "local_runners.laya.request_timeout_secs",
        );
    }
    for bad in [9, 7201, 0] {
        assert_invalid(
            &at(&format!(r#""load_timeout_secs": {bad}"#)),
            "local_runners.laya.load_timeout_secs",
        );
    }
    for (request, load_secs) in [(10, 10), (3600, 7200)] {
        let config = load_json(&at(&format!(
            r#""request_timeout_secs": {request}, "load_timeout_secs": {load_secs}"#
        )))
        .unwrap();
        let laya = config.local_runners.get("laya").unwrap();
        assert_eq!(laya.request_timeout(), Duration::from_secs(request));
        assert_eq!(laya.load_timeout(), Duration::from_secs(load_secs));
    }
}
