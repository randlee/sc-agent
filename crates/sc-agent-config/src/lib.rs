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
    #[serde(default = "load_timeout")]
    pub load_timeout_secs: u64,
}

fn load_timeout() -> u64 {
    600
}

pub const KEYS: &[Key] = &[
    Key { path: "project", env: EnvValue::String },
    Key { path: "workers", env: EnvValue::Json },
    Key { path: "max_sends", env: EnvValue::Json },
    Key { path: "request_timeout_secs", env: EnvValue::Json },
    Key { path: "local_runners", env: EnvValue::Json },
];

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

pub fn repo_root() -> Option<PathBuf> {
    let out = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).output().ok()?;
    out.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// Flags, `DECIDR_*` / `SC_PROJECT`, then the repo's and the user's files.
pub fn config(project: Option<&str>, root: Option<&Path>) -> Result<layers::Layered<Config>, Error> {
    let global = home().join(".sc-agent");
    let mut files: Vec<PathBuf> = root.map(|r| vec![r.join(".sc-agent/decidr.json"), r.join(".sc-agent/sc-agent.json")]).unwrap_or_default();
    files.extend([global.join("decidr.json"), global.join("sc-agent.json")]);
    let environment: Vec<_> = std::env::vars_os().collect();
    let flags = project.map(|p| BTreeMap::from([("project".to_owned(), Value::from(p))])).unwrap_or_default();
    let inputs = Inputs {
        files: files.into_iter().map(|path| FileLayer { path, accepts: KEYS }).collect(),
        env_prefix: "DECIDR_",
        env_aliases: &[("SC_PROJECT", "project")],
        env_accepts: KEYS,
        flags,
        flag_names: BTreeMap::from([("project".to_owned(), "--project".to_owned())]),
        keys: KEYS,
        environment: &environment,
    };
    layers::load(&inputs, serde_json::from_value::<Config>, |_| None).map_err(|e| Error::Config(e.to_string()))
}
