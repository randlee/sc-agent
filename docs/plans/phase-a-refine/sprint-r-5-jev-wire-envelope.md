---
id: r-5
phase: r
status: blocked
closure_type: boundary
target_boundary: jev
track: jev
wave: 4
branch: sprint/r-5-jev-wire-envelope
worktree: sprint/r-5-jev-wire-envelope
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-2b, r-3]
parallel_safe: []
requirements:
  - REQ-SAG-0403
  - REQ-SAG-0400
  - REQ-SAG-0102
adrs:
  - ADR-SAG-0002
  - ADR-SAG-0005
  - ADR-SAG-0019
owned_paths:
  - "crates/sc-jev/src/**"
  - "crates/sc-jev/tests/**"
  - "crates/sc-jev-backends/src/validate.rs"
  - "crates/sc-jev-backends/src/wire.rs"
  - "crates/sc-agent-types/src/judge.rs"
  - "docs/sc-jev/**"
---

# Sprint r-5 — Jev wire body as envelope; one canonical schema (D9) — BLOCKED

## Blocked on

The exact question for the TypeSafe Jev API owner:

> Does the TypeSafe Jev API accept `questions` as an **array of
> `{id, prompt, choices}`**, an **object of `{type, instructions, criteria}`**,
> or **both**? Which shape does production use today?

Record the answer, with source and date, in this doc before setting
`status: planned`.

## Outcome

`CanonicalJudgePayload` is the only schema both backend families validate,
by deserializing it. The Jev HTTP body is a JSON envelope built around the
canonical object instead of a byte-splice of `"model"`. `prompt_sha256` stays
on canonical bytes.

## Closes findings

ARCHC-002, ARCHC-003.

## Deliverables (common to both branches)

1. `sc-jev` and `sc-jev-backends` validate by
   `serde_json::from_slice::<CanonicalJudgePayload>(&bundle.bytes)` with
   `deny_unknown_fields`. No other schema check remains.
2. The Jev body is `serde_json::json!({ "model": model, <canonical fields…> })`,
   serialized by `serde_json`, or `{ "model": .., "payload": <canonical> }`
   if the vendor answer allows it. The `"model"` byte-splice is deleted.
   `JudgeOutcome.input_sha256` (r-2b) is the digest of the canonical object
   re-encoded with `canonical_json_bytes` from the envelope's payload part.
3. `test_envelope_robust_to_key_ordering`: a payload with keys `context`,
   `inputs`, `metadata` (sorting before `model`) produces a valid body.
4. Land the ADR-SAG-0005 D9 amendment and REQ-SAG-0403.

## Branch A: array `{id, prompt, choices}` only (the shape production sends)

- `CanonicalJudgePayload.questions: Vec<Question>` stays.
- Delete the object-shape acceptance path in `sc-jev` and the `sc-jev` tests
  that use it; switch those tests to the array fixture.

## Branch B: object `{type, instructions, criteria}` only, or both

- Add `#[non_exhaustive] enum QuestionSet { Choices(Vec<Question>), Criteria(CriteriaSpec) }`
  in `sc-agent-types::judge` with `#[serde(untagged)]` disallowed: use an
  explicit `"shape"` tag inside the canonical payload so the digest changes
  only for the new shape. Existing array payloads stay byte-identical (no
  tag emitted for `Choices`, enforced by the cross-impl fixture).
- `sc-jev-backends` accepts `Criteria` only if the LLM system prompt asset
  supports it. Otherwise it fails with `INPUT_INVALID` and a fix hint.
- If "both": the same as above, with both variants accepted by Jev.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0403 | Both backends deserialize `CanonicalJudgePayload`; no byte-splice; `test_envelope_robust_to_key_ordering` |
| adr:ADR-SAG-0005 | D9 amendment Active |
| boundary:jev | `jev_cross_impl` and `cross_impl_branches` digests unchanged |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev -- test_envelope_robust_to_key_ordering --exact
cargo test -p sc-jev-backends -- --nocapture
cargo test -p workspace-tests -- jev_cross_impl --nocapture
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-http -p sc-jev -p sc-jev-backends
```

## This sprint does not close

Non-check Jev API endpoints (REQ-SAG-0110).
