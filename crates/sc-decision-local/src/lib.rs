//! Local System One decision backend of sc-agent: [`LocalBackend`] with daemon discovery.
//!
//! A configured local runner (a config `local_runners` entry: kind, host, port, model,
//! key_env; REQ-SAG-0801) is a Jev-compatible daemon at `http://{host}:{port}`.
//! [`LocalBackend`] builds a
//! [`SystemOneBackend`](sc_decision_systemone::SystemOneBackend) for it, so requests and
//! replies are the same System One types as for TypeSafe Jev (ADR-SAG-0001), and adds:
//!
//! - [`LocalBackend::discover`]: one probe that says whether the daemon is up and whether the
//!   configured model is loaded ([`Availability`], [`ModelState`]), logged as
//!   `local.discover`;
//! - [`LocalBackend::wait_until_loaded`]: polls until the model is loaded, so loading never
//!   counts toward a decision's request timeout (REQ-SAG-0015);
//! - for a load-on-first-request daemon (Laya), the first requests carry the load + request
//!   deadline ([`LocalTimeouts`]).
//!
//! One sealed [`LocalDaemon`] implementation per daemon kind ([`DaemonKind`], from the
//! config's closed `RunnerKind`):
//!
//! | kind | probe | model check |
//! |---|---|---|
//! | [`Strands`] | `GET /health` | served name or checkpoint |
//! | [`Laya`] | `GET /health` (Bearer if a key is set) | in `loaded`, else loads on demand |
//! | [`Mapika`] | `GET /health` | `ok` and served name |
//! | [`Generic`] | `GET /v1/models` (Bearer if a key is set) | listed; 404/405 unlisted |
//!
//! A daemon that is not listening is reported down as [`LocalError::Http`] with cause
//! `http.network`. Discovery is a library capability: nothing in this phase calls it from
//! decide or Python. The crate is sans-IO apart from the probe sends of discovery, made on the
//! caller's [`Transport`]; it reads no config, has no built-in host or
//! port, starts no runtime, and waits only through
//! [`CancelToken::wait_until`](sc_agent_types::CancelToken::wait_until).
//!
//! [`Transport`]: sc_agent_http::Transport
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod backend;
mod daemon;
mod error;
mod generic;
mod laya;
mod mapika;
mod strands;

#[doc(inline)]
pub use backend::{LOAD_POLL_INTERVAL, LocalBackend, LocalTimeouts, MAX_LOAD_WAIT};
#[doc(inline)]
pub use daemon::{Availability, DaemonKind, LocalDaemon, ModelState};
#[doc(inline)]
pub use error::{LocalError, ParseCause};
#[doc(inline)]
pub use generic::Generic;
#[doc(inline)]
pub use laya::Laya;
#[doc(inline)]
pub use mapika::Mapika;
#[doc(inline)]
pub use strands::Strands;
