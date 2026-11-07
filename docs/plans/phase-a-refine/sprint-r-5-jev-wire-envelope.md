---
id: r-5
phase: r
status: blocked
closure_type: boundary
target_boundary: jev
track: jev
wave: 5
size: M
branch: sprint/r-5-jev-wire-envelope
worktree: sprint/r-5-jev-wire-envelope
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-2b, r-3, r-3b]
parallel_safe: []
requirements:
  - REQ-SAG-0403
  - REQ-SAG-0400
  - REQ-SAG-0401
  - REQ-SAG-0500
  - REQ-SAG-0501
  - REQ-SAG-0102
  - REQ-SAG-0606
adrs:
  - ADR-SAG-0001  # validation errors stay JevError variants mapped by the CLI/PyO3
  - ADR-SAG-0002  # Branch B / "both": r-5 lands an ADR-SAG-0002 amendment (optional criteria field)
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0012
  - ADR-SAG-0019  # wording unchanged; the ADR-SAG-0005 r-5 text says so
  - ADR-SAG-0020  # r-3 amendment: single secret read site, redaction in send_with_busy_retry; the envelope builder adds no read site
  - ADR-SAG-0025
  - ADR-SAG-0026  # HTTP half: sc-jev builds HttpRequest and sends through sc-agent-http
  - ADR-SAG-0027  # error taxonomy: INPUT_INVALID via ErrorCode; private judge module fields
owned_paths:
  - "crates/sc-jev/src/**"
  - "crates/sc-jev/tests/**"
  - "crates/sc-jev-backends/src/validate.rs"
  - "crates/sc-jev-backends/src/wire.rs"
  - "crates/sc-agent-types/src/judge.rs"
  - "docs/sc-jev/**"
  - "docs/architecture.md#ADR-SAG-0005 amendment (r-5)"
  - "docs/architecture.md#ADR index Amended-by marker (r-5) in ADR-SAG-0005"
  - "docs/architecture.md#ADR-SAG-0002 amendment (r-5) (Branch B or both only; inserted and flipped by r-5)"
  - "docs/architecture.md#ADR index row ADR-SAG-0002 (Amends cell: \"A.1 subsection (Active, r-5)\" marker; Branch B or both only)"
  - "docs/requirements.md#REQ-SAG-0403 row"
---

# Sprint r-5 — Jev wire body as envelope; one canonical schema (D9) — BLOCKED

## Blocked on

Owner decision (c) in plan.md, which waits on the TypeSafe Jev API owner's
answer to:

> Does the TypeSafe Jev API accept `questions` as an **array of
> `{id, prompt, choices}`**, an **object of `{type, instructions, criteria}`**,
> or **both**? Which shape does production use today?

Record the answer, with source and date, here and in plan.md before setting
`status: planned`. Wave 5: it follows r-3b because its validation runs the
`workspace-tests` `jev_cross_impl` target r-3b creates. **Publish gate:** no
crate is published to crates.io before this sprint lands (plan.md (c)).

## Outcome

`CanonicalJudgePayload` is the only schema both backend families validate,
by deserializing it. The Jev HTTP body is a JSON envelope built around the
canonical object instead of a byte-splice of `"model"`. `prompt_sha256` stays
on canonical bytes; `input_sha256` recovery (r-2b) is unchanged in method.

## Closes findings

ARCHC-002, ARCHC-003.

## Deliverables (common to both branches)

1. `sc-jev` and `sc-jev-backends` validate by
   `serde_json::from_slice::<CanonicalJudgePayload>(bundle.bytes())` with
   `deny_unknown_fields`. No other schema check remains.
2. The Jev body is built by `serde_json` as an object holding `model` plus
   the canonical fields (or `{ "model": .., "payload": <canonical> }` if the
   vendor allows it). The byte-splice is deleted. r-2b's recovery (parse the
   final body, remove `model` or take `payload`, re-encode, hash) is adapted
   to the new body; it stays attestation-only (ADR-SAG-0025 carve-out).
3. `test_envelope_robust_to_key_ordering` (`crates/sc-jev/tests/wire.rs`): a
   payload with keys `context`, `inputs`, `metadata` (sorting before `model`)
   produces a valid body.
4. Land the ADR-SAG-0005 r-5 amendment and REQ-SAG-0403. At flip time
   r-5 appends to the ADR-SAG-0005 r-5 bullet: "ADR-SAG-0019 is unchanged:
   the envelope is the 'API shape the evaluator expects' for Jev, and
   Haiku/Luna still embed the same canonical bytes. `input_sha256` recovery
   (ADR-SAG-0019/0025 r-2b) is unchanged in method." Plus the branch
   sentence below.
5. Errors (ADR-SAG-0001, ADR-SAG-0027): a payload that fails
   deserialization is `JevError` with `ErrorCode::InputInvalid` and a fix
   hint; no ad-hoc string. The envelope builder adds no secret read site
   and sends through `sc-agent-http` as r-3 left it (ADR-SAG-0020 r-3,
   ADR-SAG-0026 HTTP half).

## Branch A: array `{id, prompt, choices}` only

- `CanonicalJudgePayload.questions` stays as is. Delete the object-shape
  acceptance path in `sc-jev` and the tests that use it; switch those tests
  to the array fixture. `crates/sc-agent-types/src/judge.rs` is not touched.
- ADR text: the ADR-SAG-0005 r-5 bullet gains "The object-shape
  (`{type, instructions, criteria}`) acceptance path in `sc-jev` is deleted;
  `questions` is an array of `{id, prompt, choices}` only." No ADR-SAG-0002
  text changes and no ADR-SAG-0002 marker is added.

## Branch B: object `{type, instructions, criteria}` only, or both

- Additive change only (r-1 made `CanonicalJudgePayload` fields private):
  `criteria: Option<CriteriaSpec>` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`; validation
  requires exactly one of non-empty `questions` or `Some(criteria)`. An array
  payload serializes byte-identically (cross-impl fixture unchanged). No
  untagged enum, no new tag on existing payloads.
- `sc-jev-backends` accepts `criteria` only if the LLM system prompt asset
  supports it; otherwise `INPUT_INVALID` with a fix hint.
- "Both": as above, with both forms accepted by Jev.
- ADR text (Branch B and "both"): r-5 inserts and flips an ADR-SAG-0002
  amendment, and adds the marker "A.1 subsection (Active, r-5)" to the
  ADR-SAG-0002 index row Amends cell (an exception to "never adds an entry",
  like the plan review 4 exceptions in plan.md "ADR ownership"; the plan PR
  could not write it because the branch was unknown):

  > ### Amendment (Phase A.1, r-5)
  >
  > **Status:** Active (Phase A.1) — landed in r-5.
  >
  > - (Active, r-5) The canonical judge payload has an optional `criteria`
  >   field (`CriteriaSpec`, `{type, instructions, criteria}`), omitted when
  >   absent. A valid payload has exactly one of non-empty `questions` or
  >   `criteria`; anything else is `INPUT_INVALID`. A payload without
  >   `criteria` serializes byte-identically to the Phase A form, so
  >   `prompt_sha256` and the cross-language fixture are unchanged. There is
  >   one payload type (`CanonicalJudgePayload`), no untagged enum and no new
  >   tag.

  The ADR-SAG-0005 r-5 bullet names `CriteriaSpec` as part of the one
  schema.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0403 | Both backends deserialize `CanonicalJudgePayload`; no byte-splice; `test_envelope_robust_to_key_ordering` |
| adr:ADR-SAG-0005 | r-5 subsection Active, including the ADR-SAG-0019-unchanged sentence and the branch sentence (Branch A: object-shape path deleted) |
| adr:ADR-SAG-0002 | Branch B or both: r-5 subsection Active as quoted above and index marker "A.1 subsection (Active, r-5)"; `jev_cross_impl` digests unchanged for array payloads; `test_payload_requires_exactly_one_of_questions_or_criteria` rejects a payload with both `questions` and `criteria` and one with neither (`INPUT_INVALID`). Branch A: ADR-SAG-0002 text and row unchanged |
| adr:ADR-SAG-0001, 0020, 0026, 0027 | Validation failures map through `ErrorCode` (`cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact`); no `disallowed_methods` expect outside `crates/sc-agent-http/src/ureq_transport.rs` and the ADR-SAG-0028 spawn sites (`cargo test -p workspace-tests --test source_scans`); `cargo test -p sc-jev --test jev_api` green over the `sc-agent-http` transport |
| boundary:jev | `jev_cross_impl` and `cross_impl_branches` digests unchanged; `test_digest_detects_faulty_wire_builder` and `test_real_wire_builder_reports_prompt_digest` still green. Regression: REQ-SAG-0401 `cargo test -p sc-agent-http --test retry` (busy retry unchanged); REQ-SAG-0500/0501 `cargo test -p sc-jev-backends --test llm` and `cargo test -p sc-jev-backends --features test-util --test wire` (Haiku/Luna embed the same canonical bytes) |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev --features test-util --test wire -- test_envelope_robust_to_key_ordering --exact
cargo test -p sc-agent-types --test judge -- test_payload_requires_exactly_one_of_questions_or_criteria --exact   # Branch B / both only
cargo test -p sc-jev --features test-util --test wire -- test_digest_detects_faulty_wire_builder --exact
cargo test -p sc-jev --features test-util --test wire -- test_real_wire_builder_reports_prompt_digest --exact
cargo test -p sc-jev --test jev_api
cargo test -p sc-jev-backends
cargo test -p sc-jev-backends --features test-util --test wire
cargo test -p sc-jev-backends --test llm
cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact
cargo test -p sc-agent-http --test retry
cargo test -p workspace-tests --test jev_cross_impl
cargo test -p sc-jev-check --test cross_impl_branches
cargo test -p workspace-tests --test source_scans
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

## This sprint does not close

Non-check Jev API endpoints (REQ-SAG-0110).
