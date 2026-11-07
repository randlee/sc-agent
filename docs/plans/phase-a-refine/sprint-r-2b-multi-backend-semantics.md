---
id: r-2b
phase: r
status: planned
closure_type: boundary
target_boundary: jev-check
track: runtime
wave: 3
branch: sprint/r-2b-multi-backend-semantics
worktree: sprint/r-2b-multi-backend-semantics
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [r-2, r-3]
parallel_safe: [r-3b]
requirements:
  - REQ-SAG-0604
  - REQ-SAG-0605
  - REQ-SAG-0606
  - REQ-SAG-0602
  - REQ-SAG-0603
adrs:
  - ADR-SAG-0008
  - ADR-SAG-0019
  - ADR-SAG-0025
owned_paths:
  - "crates/sc-jev-check/src/run/**"
  - "crates/sc-jev-check/src/equal_input/**"
  - "crates/sc-jev-check/src/session/**"
  - "crates/sc-jev-check/tests/**"
  - "crates/sc-jev/src/check.rs"
  - "crates/sc-jev-backends/src/wire.rs"
  - "crates/sc-jev-backends/src/backend.rs"
  - "crates/sc-ai/tests/cli.rs"
  - "tests/fixtures/jev-python/**"
  - "docs/sc-jev-check/**"
---

# Sprint r-2b — multi-backend semantics and equal-input (D3)

## Outcome

Multiple backends in one run are **independent by default**. Chaining prior
answers is an explicit `ChainContext` mode recorded on the attestation. A
failing backend no longer throws away answers already paid for. `equal` is
computed from the canonical-bytes digest each backend reports from its own
wire builder, so it can now be `false`. `leak` is true when any backend failed.

## Closes findings

ARCHE-002, ARCHE-013, ARCHE-001, CR-009.

## Deliverables

1. **Mode (REQ-SAG-0605).** `RunConfig::mode(MultiBackendMode)` (type from
   r-1). `Independent`: no backend receives another's answers; the LLM system
   prompt is the ADR-SAG-0019 resolved text only. `ChainContext`: later LLM
   backends receive `render_session_context(prior Ok outcomes)` appended to the
   system prompt, in `config.backends` order. Jev never receives context. The
   mode is recorded on `EqualInputRecord.mode`. CLI flag
   `sc-ai jev run --chain-context` and Python kwarg `chain_context: bool = False`
   (thin pass-through only).
2. **Per-backend results (REQ-SAG-0604).** `run` validates every backend first
   (any refusal → `Err`, nothing sent). It then calls `judge` on each backend
   and stores `BackendStatus::Ok` or `BackendStatus::Failed { code, message }`.
   It does not stop at the first failure. If every backend failed, `run`
   returns `Err(first error)`. Otherwise it returns `Ok` with
   `equal_input.leak = results.iter().any(Failed)`. Consumers handle partial
   failure the same way: stdout prints the envelope with the successful answers,
   stderr prints the first failure's `ScAgentErrorResponse`, and the exit code /
   Python exception is code 2 / `JevError` carrying a `.response` attribute.
   The session is saved with the Ok outcomes only.
3. **Equal-input (REQ-SAG-0606).** `EqualInputRecord.backends[i].input_sha256 = outcome.input_sha256`.
   `equal = every Ok backend's input_sha256 == prompt_sha256`. Backend side:
   `sc-jev/src/check.rs` hashes the canonical object bytes it serialized into
   the body. `sc-jev-backends/src/wire.rs` hashes the exact canonical blob it
   embedded in the user message, which is also the point where it is embedded.
   Delete `equal_input::same_bytes` and the constant-`true` path.
4. **ADR text.** Land the ADR-SAG-0008 amendment, the ADR-SAG-0019
   equal-input bullet, and ADR-SAG-0025 → Active.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0604 | `test_partial_failure_keeps_answers`; `test_all_failed_returns_err`; `test_leak_true_iff_any_failed`; CLI `test_cli_partial_failure_both_streams_exit_2` |
| req:REQ-SAG-0605 | `test_independent_default_no_prior_answers_in_prompt`; `test_chain_context_recorded_and_applied` |
| req:REQ-SAG-0606 | `test_equal_false_when_backend_digest_differs` (scripted backend reports a different digest); `test_equal_true_real_wire_builders` (fake transports, Jev + Haiku + Luna) |
| adr:ADR-SAG-0008 | Amendment Active |
| adr:ADR-SAG-0019 | Equal-input bullet Active |
| adr:ADR-SAG-0025 | Status Active |
| boundary:jev-check | Regression: REQ-SAG-0602/0603 tests green; cross-impl digests unchanged |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev-check -- test_partial_failure_keeps_answers --exact
cargo test -p sc-jev-check -- test_leak_true_iff_any_failed --exact
cargo test -p sc-jev-check -- test_independent_default_no_prior_answers_in_prompt --exact
cargo test -p sc-jev-check -- test_chain_context_recorded_and_applied --exact
cargo test -p sc-jev-check -- test_equal_false_when_backend_digest_differs --exact
cargo test -p sc-jev-check -- test_equal_true_real_wire_builders --exact
cargo test -p sc-ai -- test_cli_partial_failure_both_streams_exit_2 --exact
uv run --directory tests/fixtures/jev-python pytest -q
```

## This sprint does not close

Parallel fan-out of backends (still sequential); D9 Jev envelope (r-5);
`agt-luna-live-count`.
