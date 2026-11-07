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
  - ADR-SAG-0002
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0012
  - ADR-SAG-0019
  - ADR-SAG-0025
owned_paths:
  - "crates/sc-jev/src/**"
  - "crates/sc-jev/tests/**"
  - "crates/sc-jev-backends/src/validate.rs"
  - "crates/sc-jev-backends/src/wire.rs"
  - "crates/sc-agent-types/src/judge.rs"
  - "docs/sc-jev/**"
  - "docs/architecture.md#ADR-SAG-0005 amendment (r-5)"
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
4. Land the ADR-SAG-0005 r-5 amendment and REQ-SAG-0403.

## Branch A: array `{id, prompt, choices}` only

- `CanonicalJudgePayload.questions` stays as is. Delete the object-shape
  acceptance path in `sc-jev` and the tests that use it; switch those tests
  to the array fixture. `crates/sc-agent-types/src/judge.rs` is not touched.

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

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0403 | Both backends deserialize `CanonicalJudgePayload`; no byte-splice; `test_envelope_robust_to_key_ordering` |
| adr:ADR-SAG-0005 | r-5 subsection Active |
| boundary:jev | `jev_cross_impl` and `cross_impl_branches` digests unchanged; `test_digest_detects_faulty_wire_builder` and `test_real_wire_builder_reports_prompt_digest` still green. Regression: REQ-SAG-0401 `cargo test -p sc-agent-http --test retry` (busy retry unchanged); REQ-SAG-0500/0501 `cargo test -p sc-jev-backends --test llm` and `--test wire` (Haiku/Luna embed the same canonical bytes) |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev --test wire -- test_envelope_robust_to_key_ordering --exact
cargo test -p sc-jev --test wire -- test_digest_detects_faulty_wire_builder --exact
cargo test -p sc-jev --test wire -- test_real_wire_builder_reports_prompt_digest --exact
cargo test -p sc-jev-backends
cargo test -p sc-jev-backends --test llm
cargo test -p sc-agent-http --test retry
cargo test -p workspace-tests --test jev_cross_impl
cargo test -p sc-jev-check --test cross_impl_branches
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

## This sprint does not close

Non-check Jev API endpoints (REQ-SAG-0110).
