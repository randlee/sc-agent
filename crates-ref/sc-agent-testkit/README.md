# sc-agent-testkit

Bounded test primitives for the sc-agent workspace.

**This crate is dev-only.** It has `publish = false`, and it is never a normal dependency of any crate. A crate lists it under `[dev-dependencies]` only:

```toml
[dev-dependencies]
sc-agent-testkit = { path = "../sc-agent-testkit" }
```

It depends on `sc-agent-types` only.

## Why it exists

Every wait here ends: on its event, or after `HANG_GUARD` (60 s) with a panic that names what did not happen. The guard only stops a hung test. No test proves anything by how long a wait took. A test that needs time to pass uses `sc_agent_types::testing::ManualClock`, never real time.

The workspace scan `tests_are_deterministic` (`crates/decidr/tests/workspace.rs`) refuses raw unbounded waits and clock reads in tests. This crate holds the bounded forms that the scan expects tests to use.

## What it provides

- `recv_within`, `join_within` and `cancelled_within`: a channel receive, a thread join and a wait for a cancel, each with a guard.
- `Gate` (opened by the test, or on drop of its `Opener`) and `Started` (the other thread signals it is inside): one-shot latches with bounded waits.
- `serve_once` (returns a `Server`): a loopback server for one connection, with an accept deadline and read and write timeouts.
- `serve_each` (returns a `Loopback`): a loopback server for every connection until it is stopped. `Loopback::stop` returns the number of connections made.
- `ChildGuard`: a child process that is killed and reaped on drop.
- `HANG_GUARD`.

## Usage

```rust,ignore
use std::sync::mpsc;
use std::thread;

use sc_agent_testkit::{HANG_GUARD, join_within, recv_within};

let (sender, receiver) = mpsc::channel();
let worker = thread::spawn(move || sender.send(42));

assert_eq!(recv_within(&receiver, HANG_GUARD), 42);
join_within(worker, HANG_GUARD).expect("worker finished");
```

## Depends on

- `sc-agent-types`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). Because this crate is only ever a dev-dependency, it adds no edge to any shipped crate.

## Requirements and decisions

No requirement or decision is specific to this crate. It supports the deterministic-test rule that the workspace scan enforces, and the layering of ADR-SAG-0004.
