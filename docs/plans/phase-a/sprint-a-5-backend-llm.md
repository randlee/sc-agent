---
id: a-5
phase: a
status: planned
closure_type: boundary
target_boundary: jev-backends
track: llm
wave: 2
branch: sprint/a-5-backend-llm
worktree: sprint/a-5-backend-llm
recommended_agent: tai
recommended_model: gpt-5.6-terra-medium
must_follow: [a-3]
parallel_safe: [a-4]
requirements:
  - REQ-SAG-0500
  - REQ-SAG-0501
  - REQ-SAG-0502
adrs:
  - ADR-SAG-0007
  - ADR-SAG-0019
owned_paths:
  - "crates/sc-jev-backends/src/anthropic/**"
  - "crates/sc-jev-backends/src/openai/**"
---

# Sprint a-5 — Haiku and Luna backends

## Outcome

API-direct Haiku and Luna backends: **system prompt** (evaluator role; tunable
without changing `prompt_sha256`) + **embedded canonical payload** in the judge
message; token count + send; reply validation; equal-input fields (except Jev-null
fields). HTTP body ≠ Jev wire bytes (ADR-SAG-0019).

## Deliverables

1. **REQ-SAG-0500** — Haiku wire format matches README table (fake transport tests).
2. **REQ-SAG-0501** — Luna wire format + **live** verification of `/v1/responses/input_tokens` (**requested feature 8**) documented in sprint PR evidence.
3. **REQ-SAG-0502** — LLM `INPUT_TOO_LARGE` / invalid question paths refuse send like Jev.

## Acceptance criteria

```bash
just lint
cargo test -p sc-jev-backends llm -- --nocapture
cargo test -p sc-jev-backends -- test_haiku_request_is_the_payload_alone --exact --nocapture
cargo test -p sc-jev-backends -- test_luna_request_is_the_payload_alone --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_input_is_refused_like_jev_and_nothing_is_sent --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_bad_reply_is_typed --exact --nocapture
cargo test -p sc-jev-backends -- test_unknown_question_type_has_no_answer_schema --exact --nocapture
```

Expected: exit 0 on CI (no network). PR description must attach Luna token-count probe output or state `SKIPPED: no OPENAI_API_KEY` with Rand approval.

## Parity checklist

Haiku/Luna rows + `test_equal_input_record` (with a-6 if split).

## This sprint does not close

Sessions, multi-backend CLI, PyO3.
