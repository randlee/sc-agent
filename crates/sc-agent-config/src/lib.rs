//! Layered CLI configuration and shared doctor rows through [`load_layered`].
//!
//! For each setting the first value wins: CLI flag, environment variable, local
//! `<cli>.json`, local `sc-agent.json`, global `<cli>.json`, global `sc-agent.json`,
//! then the supplied default or a missing-setting error. Global files live under
//! `<home>/.sc-agent/`; local files live under `<worktree>/.sc/sc-agent/` and are
//! skipped when there is no worktree. [`LoadOptions`] supplies the discovery inputs,
//! flags, environment and default log root. CLI settings use their CLI's environment
//! prefix; the shared project/phase uses `SC_PROJECT`. [`doctor_rows`] reports each
//! setting and its source. The loader validates values and does not write config files.
//!
//! [`AgentConfig`] holds the resolved settings, including log root, workers, send
//! limit and local decision runners. Runner credentials are resolved separately by
//! `sc_agent_secrets::resolve_var`. This crate opens no log files.
#![forbid(unsafe_code)]

mod config;
mod error;
mod layered;
pub mod layers;

#[doc(inline)]
pub use layered::{
    CliName, DoctorRow, LoadOptions, Loaded, PROJECT_ENV, SHARED_FILE, doctor_rows, load_layered,
};
#[doc(inline)]
pub use layers::{LayerFile, Source};

#[doc(inline)]
pub use config::{
    AgentConfig, DEFAULT_MAX_SENDS, DEFAULT_RUNNER_LOAD_TIMEOUT, DEFAULT_RUNNER_REQUEST_TIMEOUT,
    DEFAULT_WORKERS, ExtraModel, LocalRunner, MAX_CONFIG_FILE_BYTES, MAX_LOCAL_RUNNERS,
    MAX_MAX_SENDS, MAX_MODELS, MAX_WORKERS, RUNNER_LOAD_TIMEOUT_SECS, RUNNER_REQUEST_TIMEOUT_SECS,
    RunnerHost, RunnerKind, RunnerName, RunnerValidationError,
};
#[doc(inline)]
pub use error::AgentConfigError;
