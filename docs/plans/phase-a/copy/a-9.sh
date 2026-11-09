#!/usr/bin/env bash
# Sprint A-9 (bead agt-4p2.9): copy the sc-decision-runner starting code from sc-agent-ref@dce239a.
# Run: bash docs/plans/phase-a/copy/a-9.sh   (TARGET=<dir> to dry-run elsewhere)
#
# This script only copies text. It does NOT do any of the following; the developer does them
# (the bead design has the exact target code):
#   - write Cargo.toml, README.md, src/lib.rs, src/error.rs (RunnerError), src/runner.rs (DecisionRunner,
#     impl Decision, the per-request attempt step, the session row), src/stream.rs (ResultStream);
#   - queue.rs: keep BoundedQueue for the result queue; add JobQueue (ready FIFO + held min-heap by
#     ready_at, Condvar wait_timeout to the earliest holdoff end, cancel wakes every waiter); drop the
#     pushed/popped counters, PushError::Full and the timed push (submit always blocks; it is woken by
#     close/cancel); drop `discarded` only if the ResultStream drop path does not need it (it does: keep);
#   - pool.rs: rename WorkerPool to Pool; drop in_flight/running/alive/workers/capacity, recv_within and the
#     Counters.running field; drop the `lost` callback and the catch_unwind around `skipped` (the runner's
#     skipped fn builds a Cancelled result and cannot panic); `run` returns Step::{Done(A), HoldOff{until, job}}
#     so a busy job goes back to the JobQueue's held heap without blocking the worker;
#   - write src/config.rs (RunnerConfig) from scratch: workers come from AgentConfig.workers (A-4, default 8);
#     the old Concurrency type (stream/mod.rs:106-148, MAX 64) is not copied: no record sets a maximum;
#   - replace every thread::spawn with thread::Builder::spawn (thread::spawn panics on failure);
#   - Pool's Drop/abandon must not call cancel() on the caller's CancelToken: they cancel the JobQueue only;
#   - tests: replace the sleep-polling wait_until helper (tests.rs:66-75) and join() with channel/Rendezvous
#     signals; port the three copied tests to Pool/Step; add the holdoff, cancel-during-holdoff, session-row and
#     fastest-first tests listed in the design.
. "$(dirname "$0")/lib.sh"

S=crates/sc-jev-host/src/stream
T=crates/sc-decision-runner

copy_file  crates/sc-jev-host/LICENSE    $T/LICENSE
# Imports, Skipped, BoundedQueue (Mutex + two Condvars, no sleep), PushError, Popped
copy_lines $S/pool.rs   32 200           $T/src/queue.rs
# WorkerPool: start, submit, close_input, cancel, recv, join, abandon, Drop; Worker, Exit guard
copy_lines $S/pool.rs  202 297           $T/src/pool.rs
copy_lines $S/pool.rs  337 417           $T/src/pool.rs
# Pool tests: Rendezvous, pool_of, next, drain; parallel; cancel skips queued; panicking job answered
copy_lines $S/pool/tests.rs   1  65      $T/src/pool/tests.rs
copy_lines $S/pool/tests.rs  76  91      $T/src/pool/tests.rs
copy_lines $S/pool/tests.rs  99 125      $T/src/pool/tests.rs
copy_lines $S/pool/tests.rs 213 283      $T/src/pool/tests.rs
