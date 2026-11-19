//! Layered CLI configuration and shared doctor rows through [`load_layered`].
//! All discovery inputs and the default log root are supplied in [`LoadOptions`].
//! This crate reads layers but opens no log files and resolves no provider keys.
#![forbid(unsafe_code)]

mod config;
mod error;
mod layered;

#[doc(inline)]
pub use layered::{
    CliName, DoctorRow, LayerFile, LoadOptions, Loaded, PROJECT_ENV, SHARED_FILE, Source,
    doctor_rows, load_layered, load_layered_with_flag_names,
};

#[doc(inline)]
pub use config::{
    AgentConfig, DEFAULT_MAX_SENDS, DEFAULT_RUNNER_LOAD_TIMEOUT, DEFAULT_RUNNER_REQUEST_TIMEOUT,
    DEFAULT_WORKERS, ExtraModel, LocalRunner, MAX_CONFIG_FILE_BYTES, MAX_LOCAL_RUNNERS,
    MAX_MAX_SENDS, MAX_MODELS, MAX_WORKERS, RUNNER_LOAD_TIMEOUT_SECS, RUNNER_REQUEST_TIMEOUT_SECS,
    RunnerHost, RunnerKind, RunnerName,
};
#[doc(inline)]
pub use error::AgentConfigError;
