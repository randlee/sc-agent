//! `sc-ai` command-line adapter over [`sc_jev_check`].
//!
//! The binary parses argv and prints a versioned JSON envelope. Request
//! validation, payload assembly, prompt resolution, judge calls, sessions, and
//! key resolution stay in `sc-jev-check`. This crate does not send HTTP.
//!
//! # Session logs
//!
//! The default log root is `<repo-root>/.sc-ai`. `--log-dir` wins, then the
//! `SC_LOG_ROOT` environment variable, then that default. The v2 file sink
//! writes `<log-root>/logs/sc-ai.log.jsonl`. Session files default to
//! `<repo-root>/.sc-ai/sessions`.
//!
//! # Examples
//!
//! ```
//! use sc_ai::ENVELOPE_VERSION;
//!
//! assert_eq!(ENVELOPE_VERSION, "1");
//! ```

#![warn(missing_docs)]

mod command;
mod envelope;
mod observe;

#[doc(inline)]
pub use command::{execute, reads_stdin};
#[doc(inline)]
pub use envelope::{ENVELOPE_VERSION, OP_ASK, OP_DRY_RUN, OP_RUN};

// Test seams for `tests/cli.rs`, not part of the API. A crate cannot turn on
// its own cargo feature for its integration tests without a self
// dev-dependency, which the boundary lint and crates.io publish reject, so
// these are hidden rather than feature-gated.
#[doc(hidden)]
pub use command::{Clock, ExecuteOverrides, execute_with_overrides};
#[doc(hidden)]
pub use observe::{ObservabilitySessionLog, session_log_file};
