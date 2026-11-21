//! The decision runner of sc-agent: one [`Decision`](sc_decision_schema::Decision)
//! implementation over any [`Backend`](sc_decision_schema::Backend).
//!
//! [`DecisionRunner`] owns an `Arc<dyn Backend>`, one
//! [`HttpClient`](sc_agent_http::HttpClient) and the session and general loggers.
//! `decide_stream` starts one feeder thread and [`RunnerConfig::workers`] worker
//! threads; `decide` runs the same machinery with one request and one worker, so a
//! send never runs on the caller's thread (ADR-SAG-0001, REQ-SAG-0701).
//!
//! - A busy or timed-out send ([`Next::ResendAt`](sc_agent_http::Next::ResendAt)) and a
//!   provider pause ([`Start::Wait`](sc_agent_http::Start::Wait)) hold the job in the
//!   queue until its instant; the worker is freed (REQ-SAG-0601, REQ-SAG-0012).
//! - A reply that does not match the schema is resent unchanged, counted against the
//!   same `max_sends` and 90 s budget (REQ-SAG-0901).
//! - Cancelling the caller's [`CancelToken`](sc_agent_types::CancelToken) ends every
//!   unfinished request at once with `decision.cancelled`, in-flight sends included;
//!   a late reply is discarded.
//! - Exactly one result and one session row per request, enforced by non-`Clone`
//!   proof tokens (ADR-SAG-0002, NFR-SAG-0001).
//!
//! Sync threads, `Mutex` and `Condvar` only: no tokio, no sleep (REQ-SAG-0009,
//! REQ-SAG-0010). The crate sits in the runner layer above sc-decision-schema and
//! depends on no decision-backend crate (ADR-SAG-0004).
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod config;
mod error;
mod pool;
mod queue;
mod row;
mod runner;

#[doc(inline)]
pub use config::{JOBS_PER_WORKER, MAX_WORKERS, RunnerConfig};
#[doc(inline)]
pub use error::RunnerError;
#[doc(inline)]
pub use runner::DecisionRunner;
