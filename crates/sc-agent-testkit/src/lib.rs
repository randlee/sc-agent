//! Bounded test primitives for the sc-agent workspace (dev-only).
//!
//! Every wait here ends: on its event, or after [`HANG_GUARD`] with a panic that names what
//! did not happen. The guard is a hang guard only; no test proves anything by how long a wait
//! took. A test that needs time to pass uses a manual clock
//! (`sc_agent_types::testing::ManualClock`), never real time.
//!
//! - [`recv_within`], [`join_within`] and [`cancelled_within`]: a channel receive, a thread
//!   join and a wait for a cancel, each with a guard;
//! - [`Gate`] (opened by the test, opened on drop of its [`Opener`]) and [`Started`] (the
//!   other thread signals it is inside): one-shot latches with bounded waits;
//! - [`serve_once`]: a loopback server for one connection, with an accept deadline and
//!   read/write timeouts; [`serve_each`]: one for every connection until it is stopped, which
//!   counts every connection made (the listen backlog included);
//! - [`ChildGuard`]: a child process that is killed and reaped on drop.
//!
//! This crate depends on sc-agent-types only and is never a normal dependency of any crate.
//! The workspace scan `tests_are_deterministic` (crates/decide/tests/workspace.rs) refuses
//! raw unbounded waits and clock reads in tests; this crate is where the bounded forms live.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// This crate is test code that clippy cannot see as test code (it is a library that only
// tests link). Its interface is to fail the calling test loudly with a message: every
// primitive panics when its event does not happen within the guard, exactly as
// clippy.toml allows for tests (`allow-panic-in-tests`, `allow-expect-in-tests`).
#![allow(
    clippy::panic,
    clippy::expect_used,
    reason = "a dev-only test kit: panicking with a message is how it fails a test"
)]

mod child;
mod latch;
mod server;
mod wait;

#[doc(inline)]
pub use child::ChildGuard;
#[doc(inline)]
pub use latch::{Gate, Opener, Started};
#[doc(inline)]
pub use server::{Loopback, Server, serve_each, serve_once};
#[doc(inline)]
pub use wait::{HANG_GUARD, cancelled_within, join_within, recv_within};
