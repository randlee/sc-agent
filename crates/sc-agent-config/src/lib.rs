//! Global sc-agent configuration, loaded with sc-config from `~/.sc-agent/`.
//!
//! [`load`] reads `<config_dir>/default.json`, merges an optional
//! `local.json` and applies `SC_AGENT__*` environment overrides
//! (`SC_AGENT__MAX_SENDS=2`, `SC_AGENT__LOCAL_RUNNERS__<NAME>__PORT=<port>`).
//! Every value is checked at load (REQ-SAG-0013): a bad value is
//! [`AgentConfigError::Invalid`] with its key path, such as
//! `local_runners.laya.host`.
//!
//! The settings are extra catalog models ([`AgentConfig::catalog`]), the log
//! root ([`AgentConfig::log_root`], default `~/.sc-agent`), the worker count
//! (default 8), the send limit (default 5) and the local decision runners
//! keyed by name. This crate reads files but logs nothing and resolves no
//! keys: a runner's `key_env` is resolved by `sc_agent_secrets::resolve_var`.
#![forbid(unsafe_code)]

mod config;
mod error;

#[doc(inline)]
pub use config::{
    APP_NAME, AgentConfig, DEFAULT_MAX_SENDS, DEFAULT_RUNNER_LOAD_TIMEOUT,
    DEFAULT_RUNNER_REQUEST_TIMEOUT, DEFAULT_WORKERS, ExtraModel, LocalRunner, MAX_MAX_SENDS,
    MAX_WORKERS, RUNNER_LOAD_TIMEOUT_SECS, RUNNER_REQUEST_TIMEOUT_SECS, RunnerHost, RunnerKind,
    default_dir, load,
};
#[doc(inline)]
pub use error::AgentConfigError;
