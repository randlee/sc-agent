#![forbid(unsafe_code)]
//! Session rows (one per model request) and repo/slug identity, through an sc-observability Logger.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use sc_agent_types::Error;
use sc_observability::v2::{
    JsonlFileSink, LogEvent, Logger, LoggerBuilder, LoggerConfig, RetainedLogPolicy,
    SinkRegistration,
};
use sc_observability::{
    ActionName, ByteCount, FileCount, Level, ProcessIdentity, RetentionMaxAge, ServiceName,
    TargetCategory, Timestamp,
};
use serde_json::{Map, Value};

/// `text` if it is a slug: lower-case letters, digits and single dashes.
pub fn slug<'a>(name: &str, text: &'a str) -> Result<&'a str, Error> {
    let ok = !text.is_empty()
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.contains("--")
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(text)
    } else {
        Err(Error::Config(format!(
            "{name} must be a slug, got {text:?}"
        )))
    }
}

/// `<host>/<owner>/<repo>` from git `origin`; `local/<host>/<folder>` outside a repo.
pub fn repo_id(dir: &Path) -> String {
    let origin = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|url| !url.is_empty());
    match origin {
        Some(url) => {
            let url = url.trim_end_matches(".git");
            let url = url.split_once("://").map_or(url, |(_, rest)| rest);
            let url = url.rsplit_once('@').map_or(url, |(_, rest)| rest);
            url.replacen(':', "/", 1)
        }
        None => {
            let folder = dir
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("local/{}/{folder}", whoami_host())
        }
    }
}

fn whoami_host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "localhost".into())
}

pub struct Session {
    logger: Arc<Logger>,
    repo: String,
    project: String,
    agent_role: String,
}

impl Session {
    /// Rows to an injected Logger.
    pub fn injected(
        logger: Arc<Logger>,
        repo: String,
        project: &str,
        agent_role: &str,
    ) -> Result<Self, Error> {
        Ok(Self {
            logger,
            repo,
            project: slug("project", project)?.into(),
            agent_role: slug("agent_role", agent_role)?.into(),
        })
    }

    /// Rows to `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`, never rotated.
    pub fn open(
        log_root: &Path,
        repo: String,
        project: &str,
        agent_role: &str,
    ) -> Result<Self, Error> {
        let dir = log_root
            .join("sessions")
            .join(&repo)
            .join(slug("agent_role", agent_role)?)
            .join(slug("project", project)?);
        let keep = RetainedLogPolicy {
            rotation_max_bytes: ByteCount::from_bytes(u64::MAX),
            rotation_max_files: FileCount::from_usize(usize::MAX),
            retention_max_age: RetentionMaxAge::from_days(36_500),
            ..RetainedLogPolicy::default()
        };
        let log = |e: &dyn std::fmt::Display| Error::Log(e.to_string());
        let sink = JsonlFileSink::open(dir.join("sessions.jsonl"), keep).map_err(|e| log(&e))?;
        let mut config = LoggerConfig::default_for(
            ServiceName::new("sc-agent-session").map_err(|e| log(&e))?,
            dir.clone(),
        );
        config.enable_console_sink = false;
        config.enable_file_sink = false;
        config.retained_log_policy = keep;
        let mut builder = LoggerBuilder::new(config).map_err(|e| log(&e))?;
        builder.register_sink(SinkRegistration::typed(Arc::new(sink)));
        let logger = builder.build().map_err(|e| log(&e))?;
        Self::injected(Arc::new(logger), repo, project, agent_role)
    }

    /// Writes one row: `fields` plus repo, project and agent role. A failed write is not
    /// an error of the request it records.
    pub fn row(&self, mut fields: Map<String, Value>) {
        fields.insert("repo".into(), self.repo.clone().into());
        fields.insert("project".into(), self.project.clone().into());
        fields.insert("agent_role".into(), self.agent_role.clone().into());
        record(&self.logger, "sc_agent.session", "session.row", fields);
    }

    pub fn flush(&self) {
        let _ = self.logger.flush();
    }
}

/// One Info record on `logger`. A failed write is not an error of the call it records.
pub fn record(logger: &Logger, target: &str, action: &str, fields: Map<String, Value>) {
    let (Ok(target), Ok(action)) = (TargetCategory::new(target), ActionName::new(action)) else {
        return;
    };
    let event = LogEvent {
        version: sc_observability_types::OBSERVATION_SCHEMA_VERSION.clone(),
        timestamp: Timestamp::now_utc(),
        level: Level::Info,
        service: logger.service_name().clone(),
        target,
        action,
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields,
    };
    let _ = logger.log(event);
}

/// The log root of the entry point `name` (ADR-SAG-0021): `<NAME>_LOG_ROOT` when set and not
/// empty, else `~/.sc-agent`; an error when neither is known. The only function that computes
/// a log root: the CLIs and the wheel call it.
pub fn log_root(name: &str) -> Result<PathBuf, Error> {
    let var = root_var(name);
    root_from(&var, std::env::var_os(&var), std::env::home_dir())
}

/// `<NAME>_LOG_ROOT`: `name` upper-cased, `-` as `_`.
fn root_var(name: &str) -> String {
    format!("{}_LOG_ROOT", name.to_ascii_uppercase().replace('-', "_"))
}

fn root_from(var: &str, set: Option<OsString>, home: Option<PathBuf>) -> Result<PathBuf, Error> {
    match set.filter(|v| !v.is_empty()) {
        Some(root) => Ok(PathBuf::from(root)),
        None => home
            .filter(|h| !h.as_os_str().is_empty())
            .map(|h| h.join(".sc-agent"))
            .ok_or_else(|| Error::Config(format!("the home directory is unknown; set {var}"))),
    }
}

/// The general Logger of `service`: `<log_root>/logs/<service>.log.jsonl`, no console.
pub fn general(service: &str, log_root: &Path) -> Result<Arc<Logger>, Error> {
    let log = |e: &dyn std::fmt::Display| Error::Log(e.to_string());
    let mut config = LoggerConfig::default_for(
        ServiceName::new(service).map_err(|e| log(&e))?,
        log_root.to_path_buf(),
    );
    config.enable_console_sink = false;
    Ok(Arc::new(
        LoggerBuilder::new(config)
            .map_err(|e| log(&e))?
            .build()
            .map_err(|e| log(&e))?,
    ))
}

/// Rows are written in the background; a dropped session waits for them.
impl Drop for Session {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod root_tests {
    use super::*;

    #[test]
    fn the_variable_wins_when_set_and_not_empty() {
        let home = Some(PathBuf::from("/h"));
        assert_eq!(root_var("plan-anchor"), "PLAN_ANCHOR_LOG_ROOT");
        let set = Some(OsString::from("/r"));
        assert_eq!(
            root_from("V", set, home.clone()).ok(),
            Some(PathBuf::from("/r"))
        );
        let empty = Some(OsString::new());
        let fallback = root_from("V", empty, home).ok();
        assert_eq!(fallback, Some(PathBuf::from("/h/.sc-agent")));
    }

    #[test]
    fn no_home_and_no_variable_is_an_error_naming_the_variable() {
        let error = root_from("SITR_LOG_ROOT", None, Some(PathBuf::new())).err();
        assert!(error.is_some_and(|e| e.to_string().contains("SITR_LOG_ROOT")));
    }
}
