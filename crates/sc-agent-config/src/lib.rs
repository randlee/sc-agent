#![forbid(unsafe_code)]
//! decidr settings through sc-config layers: flags, environment, then repo and user files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sc_agent_types::Error;
use sc_config::layers::{self, EnvValue, FileLayer, Inputs, Key};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub project: Option<String>,
    pub workers: Option<usize>,
    pub max_sends: Option<u32>,
    pub request_timeout_secs: Option<u64>,
    #[serde(default)]
    pub local_runners: BTreeMap<String, Runner>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub model: Option<String>,
    #[serde(default = "request_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "load_timeout")]
    pub load_timeout_secs: u64,
}

fn load_timeout() -> u64 {
    600
}

fn request_timeout() -> u64 {
    300
}

pub const KEYS: &[Key] = &[
    Key {
        path: "project",
        env: EnvValue::String,
    },
    Key {
        path: "workers",
        env: EnvValue::Json,
    },
    Key {
        path: "max_sends",
        env: EnvValue::Json,
    },
    Key {
        path: "request_timeout_secs",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners.*",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners.*.kind",
        env: EnvValue::String,
    },
    Key {
        path: "local_runners.*.host",
        env: EnvValue::String,
    },
    Key {
        path: "local_runners.*.port",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners.*.model",
        env: EnvValue::String,
    },
    Key {
        path: "local_runners.*.request_timeout_secs",
        env: EnvValue::Json,
    },
    Key {
        path: "local_runners.*.load_timeout_secs",
        env: EnvValue::Json,
    },
];

pub fn home() -> Result<PathBuf, Error> {
    checked_home(std::env::home_dir())
}

fn checked_home(home: Option<PathBuf>) -> Result<PathBuf, Error> {
    home.filter(|path| path.is_absolute()).ok_or_else(|| {
        Error::Config("an absolute home directory is required to read global configuration".into())
    })
}

pub fn repo_root() -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// Flags, `DECIDR_*` / `SC_PROJECT`, then the repo's and the user's files.
pub fn config(
    project: Option<&str>,
    root: Option<&Path>,
) -> Result<layers::Layered<Config>, Error> {
    let global = home()?.join(".sc-agent");
    let mut files: Vec<PathBuf> = root
        .map(|r| {
            vec![
                r.join(".sc-agent/decidr.json"),
                r.join(".sc-agent/sc-agent.json"),
            ]
        })
        .unwrap_or_default();
    files.extend([global.join("decidr.json"), global.join("sc-agent.json")]);
    let environment: Vec<_> = std::env::vars_os().collect();
    let flags = project
        .map(|p| BTreeMap::from([("project".to_owned(), Value::from(p))]))
        .unwrap_or_default();
    let inputs = Inputs {
        files: files
            .into_iter()
            .map(|path| FileLayer {
                path,
                accepts: KEYS,
            })
            .collect(),
        env_prefix: "DECIDR_",
        env_aliases: &[("SC_PROJECT", "project")],
        env_accepts: KEYS,
        flags,
        flag_names: BTreeMap::from([("project".to_owned(), "--project".to_owned())]),
        keys: KEYS,
        environment: &environment,
    };
    layers::load(&inputs, decode, |e| {
        e.strip_prefix("invalid ")
            .and_then(|s| s.split_once(':').map(|(path, _)| path))
    })
    .map_err(|e| Error::Config(e.to_string()))
}

fn decode(value: Value) -> Result<Config, String> {
    let config: Config = serde_json::from_value(value).map_err(|e| e.to_string())?;
    if config.workers.is_some_and(|v| !(1..=64).contains(&v)) {
        return Err("invalid workers: out of range".into());
    }
    if config.max_sends.is_some_and(|v| !(1..=10).contains(&v)) {
        return Err("invalid max_sends: out of range".into());
    }
    for (name, r) in &config.local_runners {
        for (field, valid) in [
            ("port", r.port != 0),
            (
                "host",
                r.host.parse::<std::net::IpAddr>().is_ok()
                    || (!r.host.is_empty()
                        && !r
                            .host
                            .chars()
                            .any(|c| c.is_whitespace() || ":/?#@".contains(c))),
            ),
            (
                "load_timeout_secs",
                (10..=7200).contains(&r.load_timeout_secs),
            ),
            (
                "request_timeout_secs",
                (10..=3600).contains(&r.request_timeout_secs),
            ),
            (
                "kind",
                matches!(r.kind.as_str(), "generic" | "laya" | "strands" | "mapika"),
            ),
        ] {
            if !valid {
                return Err(format!(
                    "invalid local_runners.{name}.{field}: invalid value"
                ));
            }
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn local_runner_fields_load_from_explicit_nested_environment() {
        let environment = [
            (
                OsString::from("DECIDR_LOCAL_RUNNERS__TEST__KIND"),
                OsString::from("generic"),
            ),
            (
                OsString::from("DECIDR_LOCAL_RUNNERS__TEST__HOST"),
                OsString::from("127.0.0.1"),
            ),
            (
                OsString::from("DECIDR_LOCAL_RUNNERS__TEST__PORT"),
                OsString::from("12345"),
            ),
        ];
        let inputs = Inputs {
            files: Vec::new(),
            env_prefix: "DECIDR_",
            env_aliases: &[],
            env_accepts: KEYS,
            flags: BTreeMap::new(),
            flag_names: BTreeMap::new(),
            keys: KEYS,
            environment: &environment,
        };

        let loaded = layers::load(&inputs, decode, |_| None)
            .expect("valid local runner environment is accepted");
        let runner = loaded
            .config
            .local_runners
            .get("test")
            .expect("runner is loaded");
        assert_eq!(runner.kind, "generic");
        assert_eq!(runner.host, "127.0.0.1");
        assert_eq!(runner.port, 12345);
        assert_eq!(runner.load_timeout_secs, 600);
        assert_eq!(runner.request_timeout_secs, 300);
        assert!(decode(serde_json::json!({"local_runners":{"test":{"kind":"generic","host":"localhost","port":1,"request_timeout_secs":9}}})).unwrap_err().contains("local_runners.test.request_timeout_secs"));
        assert!(decode(serde_json::json!({"local_runners":{"test":{"kind":"generic","host":"localhost","port":0}}})).unwrap_err().contains("local_runners.test.port"));
        let invalid = [("DECIDR_WORKERS".into(), "0".into())];
        let inputs = Inputs {
            environment: &invalid,
            ..inputs
        };
        let error = layers::load(&inputs, decode, |e| {
            e.strip_prefix("invalid ")
                .and_then(|s| s.split_once(':').map(|(path, _)| path))
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("DECIDR_WORKERS"));
    }
}

#[cfg(test)]
mod home_tests {
    use super::*;

    #[test]
    fn missing_empty_and_relative_homes_are_errors() {
        for home in [
            None,
            Some(PathBuf::new()),
            Some(PathBuf::from("relative/home")),
        ] {
            assert!(matches!(checked_home(home), Err(Error::Config(message))
                if message.contains("absolute home directory")));
        }
    }

    #[test]
    fn an_absolute_home_is_preserved() {
        let absolute = PathBuf::from(if cfg!(windows) {
            r"C:\controlled-home"
        } else {
            "/controlled-home"
        });
        assert!(absolute.is_absolute());
        assert_eq!(checked_home(Some(absolute.clone())).ok(), Some(absolute));
    }
}
