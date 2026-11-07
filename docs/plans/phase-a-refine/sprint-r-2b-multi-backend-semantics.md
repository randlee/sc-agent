---
id: r-2b
phase: r
status: planned
closure_type: boundary
target_boundary: jev-check
track: runtime
wave: 3
size: L
branch: sprint/r-2b-multi-backend-semantics
worktree: sprint/r-2b-multi-backend-semantics
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [r-2, r-3]
parallel_safe: []
vertical_rationale: "Equal-input is a two-sided contract: the engine compares digests that each backend crate must recover from its own final wire body. Backend-side hashing is owned only here (r-1 added the Option field; r-3 built the final body)."
requirements:
  - REQ-SAG-0606
  - REQ-SAG-0604
  - REQ-SAG-0605
  - REQ-SAG-0602
  - REQ-SAG-0603
  - REQ-SAG-0610
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0710
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0007
  - ADR-SAG-0008
  - ADR-SAG-0012
  - ADR-SAG-0019
  - ADR-SAG-0022
  - ADR-SAG-0024
  - ADR-SAG-0025
owned_paths:
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-jev-check/src/run/**"
  - "crates/sc-jev-check/src/equal_input/**"
  - "crates/sc-jev-check/src/session/**"
  - "crates/sc-jev-check/src/registry/scripted.rs"
  - "crates/sc-jev-check/tests/**"
  - "crates/sc-jev/Cargo.toml"
  - "crates/sc-jev/src/check.rs"
  - "crates/sc-jev/src/lib.rs"
  - "crates/sc-jev/tests/**"
  - "crates/sc-jev-backends/Cargo.toml"
  - "crates/sc-jev-backends/src/wire.rs"
  - "crates/sc-jev-backends/src/backend.rs"
  - "crates/sc-jev-backends/src/lib.rs"
  - "crates/sc-jev-backends/tests/**"
  - "boundaries/sc-jev/**"
  - "boundaries/sc-jev-backends/**"
  - "docs/sc-jev-check/**"
  - "docs/architecture.md#ADR-SAG-0004 amendment (r-2b)"
  - "docs/architecture.md#ADR-SAG-0008 amendment (r-2b)"
  - "docs/architecture.md#ADR-SAG-0019 amendment (r-2b)"
  - "docs/architecture.md#ADR-SAG-0022 amendment (r-2b)"
  - "docs/architecture.md#ADR-SAG-0025"
  - "docs/architecture.md#ADR index row ADR-SAG-0025"
  - "docs/architecture.md#ADR index Amended-by marker (r-2b) in ADR-SAG-0004, 0008, 0019"
  - "docs/requirements.md#REQ-SAG-0606 row"
---

# Sprint r-2b — multi-backend semantics and equal-input (D3, engine only)

## Outcome

In the engine: multiple backends in one run are **independent by default**;
`ChainContext` is explicit and recorded; a failing backend no longer throws
away answers already paid for; `equal` is computed from the digest each
backend recovers from its own final wire body, so it can be `false`; `leak`
is true when any backend failed. Consumer-visible behaviour (CLI flag, exit
codes, Python kwarg, `.response`) is r-3b's; this sprint changes no file in
`sc-ai` or `sc-jev-py`.

## Closes findings

ARCHE-001, ARCHE-002 (engine), ARCHE-013, CR-009.

## Deliverables

1. **Mode (ADR-SAG-0008, REQ-SAG-0605 engine part).** `RunConfig::mode`
   honoured. `Independent`: no backend receives another's answers; the LLM
   system prompt is the ADR-SAG-0019 resolved text only. `ChainContext`: later
   LLM backends receive `render_session_context(prior Ok outcomes)` appended to
   the system prompt, in `BackendSet` order; Jev never receives context. The
   mode is recorded on `EqualInputRecord.mode`.
2. **Validation per send (ADR-SAG-0004 amendment, CPR-006, OBS-A).** Before
   the first send, `run` validates the base request on every backend (any
   refusal → `Err`, nothing sent). In `ChainContext`, each augmented request is
   re-validated with `validate_for_send` immediately before its own send; a
   refusal there becomes that backend's `BackendStatus::Failed`
   (`INPUT_TOO_LARGE`, `leak = true`) and no request is sent for it.
3. **Per-backend results (REQ-SAG-0604 engine part).** `run` calls `judge`
   once per backend and records `Ok` or `Failed(BackendFailure)` (r-2 type:
   stores the `JevError`; the serialized `ScAgentErrorResponse` is derived). It does not stop at the
   first failure. If every backend failed, `run` returns `Err(first error)`;
   otherwise `Ok` with `leak = results.iter().any(Failed)`.
4. **Paid-call rules (OBS-B).** `judge` is called exactly once per backend
   per run; the only retries are `sc-agent-http` `BusyPolicy` retries on
   429/502/529; a transport error or timeout after the body was sent is never
   re-sent. If saving the session fails after paid answers, `run` returns `Ok`
   with `session_saved = false`, `session_id = None` and one entry in
   `RunResponse::warnings` (the r-2 field: `"session not saved: <code>"`, no
   path or secret), and the answers are kept; the bytes match r-2's pinned
   `session_save_failed.json`. Tests: `test_no_resend_on_timeout`,
   `test_session_save_failure_keeps_answers`.
5. **Equal-input (REQ-SAG-0606, ADR-SAG-0019 r-2b amendment).** Each backend
   sets `JudgeOutcome::with_input_sha256` to the SHA-256 of the canonical blob
   **recovered from the final serialized `HttpRequest.body`** it handed to
   `send_with_busy_retry`: `sc-jev/src/check.rs` parses the body, removes
   `model`, re-encodes with `canonical_json_bytes`, hashes;
   `sc-jev-backends/src/wire.rs` parses the body, extracts the embedded
   canonical blob from the user message, re-encodes, hashes. The engine maps
   each result to `Attestation::{Matched, Mismatched { reported }, Unattested, Failed}`;
   `equal` is true only if every entry is `Matched`. Delete
   `equal_input::same_bytes` and the constant-`true` path. The scripted backend
   (`crates/sc-jev-check/src/registry/scripted.rs`, ADR-SAG-0022 r-2b
   amendment) builds over r-1's `MockJudgeBackend`, so it reports the digest
   of the bundle it received; `test_equal_false_when_backend_digest_differs`
   uses `MockJudgeBackend::with_reported_digest`.
6. **Wire-level digest tests on real backend types (CPR-005, CPR-108).**
   Test seam per the plan convention: `sc-jev` and `sc-jev-backends` each
   gain a non-default `test-util` feature exposing
   `WireFault::DropEmbeddedByte` and a `with_wire_fault(WireFault)` builder
   method on `JevClient` / `AnthropicHaikuBackend` / `OpenAiLunaBackend`; the
   fault is applied inside the real wire builder after canonical bytes are
   embedded. Each crate's own integration tests enable it through a self
   dev-dependency; transports come from `sc-agent-http`'s `test-util` test
   transport (dev-dependency). Both boundary manifests name the feature in
   `notes`. Tests live **in each backend crate** (no new dependency edge
   from `sc-jev-check` to `sc-agent-http`):
   - `test_real_wire_builder_reports_prompt_digest`
     (`crates/sc-jev/tests/wire.rs` for Jev; `crates/sc-jev-backends/tests/wire.rs`
     for Haiku and Luna): with no fault, the outcome's `input_sha256` equals
     the request's `prompt_sha256`.
   - `test_digest_detects_faulty_wire_builder` (same two files): with
     `WireFault::DropEmbeddedByte`, the reported digest ≠ `prompt_sha256`.
   The engine mapping of a reported digest to `Matched` / `Mismatched` is
   proven once in `sc-jev-check` with the mock
   (`test_equal_false_when_backend_digest_differs`,
   `test_equal_true_when_all_digests_match`).
7. **ADR text.** ADR-SAG-0025 → Active (header "Amends ADR-SAG-0004,
   ADR-SAG-0008, ADR-SAG-0019"; states the engine trusts the backend's
   reported digest). ADR-SAG-0008, ADR-SAG-0004, ADR-SAG-0019 and
   ADR-SAG-0022 r-2b amendment subsections → Active. ADR index row
   ADR-SAG-0025 and the r-2b "Amended by" markers in rows 0004, 0008, 0019
   → Active.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0606 | `test_equal_false_when_backend_digest_differs` (mock reports another digest); `test_equal_true_when_all_digests_match`; `test_real_wire_builder_reports_prompt_digest` and `test_digest_detects_faulty_wire_builder` in `sc-jev` (Jev) and `sc-jev-backends` (Haiku, Luna) |
| adr:ADR-SAG-0025 | Status Active; index row Active |
| adr:ADR-SAG-0022 | r-2b subsection Active (scripted backend reports the received digest) |
| adr:ADR-SAG-0005, 0007 | Regression: backends still do not re-canonicalize the wire body (digest re-encoding is attestation-only and never feeds the wire body; the byte-exact Jev body regression `cargo test -p sc-jev --test jev_api` and `cargo test -p sc-jev-backends --test llm` stay green); retries only via `BusyPolicy` (`test_no_resend_on_timeout`) |
| adr:ADR-SAG-0004 | `test_chain_context_augmented_request_revalidated` (over-cap chained request refused before send; that backend `Failed`, `leak = true`) |
| adr:ADR-SAG-0008 | `test_independent_default_no_prior_answers_in_prompt`; `test_chain_context_recorded_and_applied` |
| boundary:jev-check | `test_partial_failure_keeps_answers`; `test_all_failed_returns_err`; `test_leak_true_iff_any_failed`; `test_no_resend_on_timeout`; `test_session_save_failure_keeps_answers`; regression REQ-SAG-0602/0603/0610 tests; cross-impl digests unchanged; `test-util` absent from `cargo tree -p sc-jev -e normal,features` and the same for `sc-jev-backends` |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev-check --test multi_backend -- test_partial_failure_keeps_answers --exact
cargo test -p sc-jev-check --test multi_backend -- test_all_failed_returns_err --exact
cargo test -p sc-jev-check --test multi_backend -- test_leak_true_iff_any_failed --exact
cargo test -p sc-jev-check --test multi_backend -- test_independent_default_no_prior_answers_in_prompt --exact
cargo test -p sc-jev-check --test multi_backend -- test_chain_context_recorded_and_applied --exact
cargo test -p sc-jev-check --test multi_backend -- test_chain_context_augmented_request_revalidated --exact
cargo test -p sc-jev-check --test multi_backend -- test_no_resend_on_timeout --exact
cargo test -p sc-jev-check --test multi_backend -- test_session_save_failure_keeps_answers --exact
cargo test -p sc-jev-check --test multi_backend -- test_equal_false_when_backend_digest_differs --exact
cargo test -p sc-jev-check --test multi_backend -- test_equal_true_when_all_digests_match --exact
cargo test -p sc-jev --test wire -- test_real_wire_builder_reports_prompt_digest --exact
cargo test -p sc-jev --test wire -- test_digest_detects_faulty_wire_builder --exact
cargo test -p sc-jev-backends --test wire -- test_real_wire_builder_reports_prompt_digest --exact
cargo test -p sc-jev-backends --test wire -- test_digest_detects_faulty_wire_builder --exact
cargo test -p sc-jev-check --test run_response -- test_run_response_contract --exact
cargo test -p sc-jev --test jev_api
cargo test -p sc-jev-backends --test llm
test -z "$(cargo tree -p sc-jev -e normal,features | grep -F '"test-util"')"
test -z "$(cargo tree -p sc-jev-backends -e normal,features | grep -F '"test-util"')"
! rg -n '#!?\[allow\(' crates/
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

`sc-ai jev run --chain-context`, partial-failure stdout/stderr/exit 2, the
Python `chain_context` kwarg, `JevError.response`, `.pyi` (r-3b, which closes
REQ-SAG-0604 and REQ-SAG-0605); parallel fan-out of backends (still
sequential); D9 Jev envelope (r-5); `agt-luna-live-count`.
