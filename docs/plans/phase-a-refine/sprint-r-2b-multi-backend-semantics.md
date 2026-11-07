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
  - REQ-SAG-0710
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0012
  - ADR-SAG-0014
  - ADR-SAG-0019
  - ADR-SAG-0022
  - ADR-SAG-0024
  - ADR-SAG-0025
  - ADR-SAG-0026  # HTTP half only (Active after r-3); r-2b relies on no compose-half bullet
  - ADR-SAG-0027
  - ADR-SAG-0029
owned_paths:
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-jev-check/src/run/**"
  - "crates/sc-jev-check/src/equal_input/**"
  - "crates/sc-jev-check/src/session/**"
  - "crates/sc-jev-check/src/registry/scripted.rs"
  - "crates/sc-jev-check/tests/**"
  - "crates/sc-jev/Cargo.toml"
  - "crates/sc-jev/src/check.rs"
  - "crates/sc-jev/src/client/**"
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
  - "docs/architecture.md#ADR index Amended-by marker (r-2b) in ADR-SAG-0004, 0008, 0019, 0022"
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

1. **Mode (ADR-SAG-0008, REQ-SAG-0605 engine part).**
   `RunConfig::mode(self, mode: MultiBackendMode) -> Self` is **added here**
   (r-2 ships no `mode` setter, because its only governing text is
   ADR-SAG-0025) and honoured. `Independent`: no backend receives another's answers; the LLM
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
   `equal_input::same_bytes` and the constant-`true` path. `JevClient` (its
   struct, constructor and `impl JudgeBackend`) is in
   `crates/sc-jev/src/client/mod.rs`, which r-3 keeps when it deletes
   `client/{transport,retry,https}.rs`; r-2b owns `client/**` for
   `with_input_sha256` and `with_wire_fault`. The scripted backend
   (`crates/sc-jev-check/src/registry/scripted.rs`, ADR-SAG-0022 r-2b
   amendment) builds over r-1's `MockJudgeBackend`, so it reports the digest
   of the bundle it received; `test_equal_false_when_backend_digest_differs`
   uses `MockJudgeBackend::with_reported_digest`.

   **Scripted-answers schema** (CPR-208; the one format behind
   `register_scripted_backends(ids, answers: Value)`, r-3b's hidden
   `--scripted-answers <file>` flag and `sc_jev_py._testing.run_check_scripted`).
   The value is one of two JSON shapes:
   - *Shared form* (a-10, unchanged): an object of question id → answer,
     where an answer is a choice string (`"yes"`, the ADR-SAG-0029 (b)
     shorthand) or an answer object (`{"choice", "confidence"?,
     "probabilities"?}`); every id in `ids` answers with it.
   - *Per-backend form*: an object whose only key is `"backends"`, mapping a
     backend id (descriptor `name()`) to either a shared-form answer object
     or a failure entry `{"failure": {"error": "<ErrorCode>", "kind": "<NetworkErrorKind>", "stage": "judge" | "validate"}}`.
     `stage: "judge"` (default) maps to `MockJudgeBackend::with_failure`, so
     the backend is sent and fails; `stage: "validate"` maps to
     `MockJudgeBackend::refusing`, so nothing is sent. `error` is one of
     `NETWORK_ERROR` (requires `kind`, e.g. `"busy"`), `INPUT_TOO_LARGE`,
     `RESPONSE_INVALID`; anything else, a backend id not in `ids`, an id in
     `ids` with no entry, or a shared-form map whose only question id is
     `backends` is `INPUT_INVALID` with a fix hint naming the two forms.
   Example: `{"backends": {"jev": {"failure": {"error": "NETWORK_ERROR", "kind": "busy"}}, "haiku": {"relevant": "yes"}}}`.
   Tests (target `scripted`, declared
   `[[test]] required-features = ["test-util"]`; validation lines pass
   `--features test-util`): `test_scripted_shared_form_unchanged`,
   `test_scripted_per_backend_failure` (one backend `Failed` with
   `NETWORK_ERROR`/`Busy`, the other `Ok`), `test_scripted_validate_stage_not_sent`
   (`NotSent`, nothing recorded by the mock), `test_scripted_invalid_form_rejected`.
6. **Wire-level digest tests on real backend types (CPR-005, CPR-108).**
   Test seam per the plan convention: `sc-jev` and `sc-jev-backends` each
   gain a non-default `test-util` feature exposing
   `WireFault::DropEmbeddedByte` and a `with_wire_fault(WireFault)` builder
   method on `JevClient` / `AnthropicHaikuBackend` / `OpenAiLunaBackend`; the
   fault is applied inside the real wire builder after canonical bytes are
   embedded. Each crate's `wire` target is declared
   `[[test]] required-features = ["test-util"]` and its validation lines pass
   `--features test-util`; there is no self dev-dependency (r-1's D0 spike
   result, `boundaries/README.md` "Dev-dependency and self edges"); transports come from `sc-agent-http`'s `test-util` test
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
   ADR-SAG-0008, ADR-SAG-0019, ADR-SAG-0022"; states the engine trusts the
   backend's reported digest). ADR-SAG-0008, ADR-SAG-0004, ADR-SAG-0019 and
   ADR-SAG-0022 r-2b amendment subsections → Active (the 0022 subsection
   records the digest rule and the per-backend scripted-answers form; it
   supersedes no ADR-SAG-0029 sentence, and 0029 (b)'s shorthand rule stands
   for the shared form). ADR index row ADR-SAG-0025 and the r-2b "Amended by"
   markers in rows 0004, 0008, 0019, 0022 → Active.

   **ADR-SAG-0025 bullet 2 split at flip time** (plan review 4, pre-screen
   r-2b). Bullet 2's clause "REQ-SAG-0604 (consumer output in r-3b: stdout
   envelope, stderr first failure, exit 2)" would go Active at r-2b merge
   but is delivered by r-3b. r-2b ends bullet 2 at "…and one `warnings`
   entry. REQ-SAG-0604 (engine part)." and inserts, directly after it, an
   r-3b-owned bullet that it leaves Proposed:

   > - (Proposed, r-3b) REQ-SAG-0604 consumer output: on a partial result
   >   (`RunResponse::is_partial()`) `sc-ai` prints the envelope with the
   >   successful answers on stdout, the first failure's
   >   `ScAgentErrorResponse` on stderr, and exits 2; `sc-jev-py` raises
   >   `JevError` with a `.response` attribute holding the envelope `data`.
   >   All-failed stays a plain error.

   The ADR-SAG-0025 Status line then reads "Active (Phase A.1, r-2b); the
   REQ-SAG-0604 consumer-output bullet is Proposed (r-3b)", and the index
   row Status cell "Active (r-2b); consumer-output bullet Proposed (r-3b)".
   r-3b flips that bullet, the Status line and the cell to Active.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0606 | `test_equal_false_when_backend_digest_differs` (mock reports another digest); `test_equal_true_when_all_digests_match`; `test_real_wire_builder_reports_prompt_digest` and `test_digest_detects_faulty_wire_builder` in `sc-jev` (Jev) and `sc-jev-backends` (Haiku, Luna) |
| adr:ADR-SAG-0025 | Status line and index row read as above (Active, r-2b; consumer-output bullet Proposed, r-3b); bullet 2 ends at "REQ-SAG-0604 (engine part)" and no Active bullet names r-3b consumer behaviour; bullet 1 matches `RunConfig::mode` as added in deliverable 1 |
| adr:ADR-SAG-0006, 0009, 0014, 0026, 0027 | Regression: backends still take their transport through the r-3 `Transport` generic (0006, 0026 HTTP half; `cargo test -p sc-jev --test jev_api` and `cargo test -p sc-jev-backends --test llm`); the scripted registry still installs over existing ids only and the second scripted-answers form goes through `register_scripted_backends` (0009); `RunResponse` serialization and the session-save `warnings` entry match the r-2 `data` body (0014, `test_run_response_contract`); `Attestation` and `input_sha256` keep the r-1 types and every new failure maps through `ErrorCode` (0027, `test_error_response_stderr_golden`) |
| adr:ADR-SAG-0022 | r-2b subsection Active (scripted backend reports the received digest; per-backend scripted-answers form); `test_scripted_shared_form_unchanged`, `test_scripted_per_backend_failure`, `test_scripted_validate_stage_not_sent`, `test_scripted_invalid_form_rejected` |
| adr:ADR-SAG-0005, 0007 | Regression: backends still do not re-canonicalize the wire body (digest re-encoding is attestation-only and never feeds the wire body; the byte-exact Jev body regression `cargo test -p sc-jev --test jev_api` and `cargo test -p sc-jev-backends --test llm` stay green); retries only via `BusyPolicy` (`test_no_resend_on_timeout`) |
| adr:ADR-SAG-0004 | `test_chain_context_augmented_request_revalidated` (over-cap chained request refused before send; that backend `Failed`, `leak = true`) |
| adr:ADR-SAG-0008 | `test_independent_default_no_prior_answers_in_prompt`; `test_chain_context_recorded_and_applied` |
| boundary:jev-check | `test_partial_failure_keeps_answers`; `test_all_failed_returns_err`; `test_leak_true_iff_any_failed`; `test_no_resend_on_timeout`; `test_session_save_failure_keeps_answers`; cross-impl digests unchanged; `test-util` absent from `cargo tree -p sc-jev -e normal,features` and the same for `sc-jev-backends`. Regression per listed id: REQ-SAG-0602/0603 `cargo test -p sc-jev-check --features test-util --test multi_backend`; REQ-SAG-0610 `cargo test -p sc-jev-check --lib -- run::tests::`; REQ-SAG-0710 `test_run_response_contract`; NFR-SAG-0001 `cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact` (`results[].error`); NFR-SAG-0002 `cargo test -p sc-jev-backends --test llm`; NFR-SAG-0003 `cargo test -p sc-jev-check --features test-util --test sessions_exact` |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_partial_failure_keeps_answers --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_all_failed_returns_err --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_leak_true_iff_any_failed --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_independent_default_no_prior_answers_in_prompt --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_chain_context_recorded_and_applied --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_chain_context_augmented_request_revalidated --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_no_resend_on_timeout --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_session_save_failure_keeps_answers --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_equal_false_when_backend_digest_differs --exact
cargo test -p sc-jev-check --features test-util --test multi_backend -- test_equal_true_when_all_digests_match --exact
cargo test -p sc-jev --features test-util --test wire -- test_real_wire_builder_reports_prompt_digest --exact
cargo test -p sc-jev --features test-util --test wire -- test_digest_detects_faulty_wire_builder --exact
cargo test -p sc-jev-backends --features test-util --test wire -- test_real_wire_builder_reports_prompt_digest --exact
cargo test -p sc-jev-backends --features test-util --test wire -- test_digest_detects_faulty_wire_builder --exact
cargo test -p sc-jev-check --features test-util --test scripted -- test_scripted_shared_form_unchanged --exact
cargo test -p sc-jev-check --features test-util --test scripted -- test_scripted_per_backend_failure --exact
cargo test -p sc-jev-check --features test-util --test scripted -- test_scripted_validate_stage_not_sent --exact
cargo test -p sc-jev-check --features test-util --test scripted -- test_scripted_invalid_form_rejected --exact
cargo test -p sc-jev-check --features test-util --test run_response -- test_run_response_contract --exact
cargo test -p sc-jev-check --features test-util --test multi_backend
cargo test -p sc-jev-check --lib -- run::tests::
cargo test -p sc-jev-check --features test-util --test sessions_exact
cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact
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
