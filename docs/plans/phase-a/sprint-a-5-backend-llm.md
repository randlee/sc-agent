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
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-4]
requirements:
  - REQ-SAG-0500
  - REQ-SAG-0501
  - REQ-SAG-0502
  - REQ-SAG-0503
adrs:
  - ADR-SAG-0007
  - ADR-SAG-0019
  - ADR-SAG-0006
owned_paths:
  - "crates/sc-jev-backends/**"
  - "boundaries/jev-backends/**"
---

# Sprint a-5 — Haiku and Luna backends

## Outcome

API-direct Haiku and Luna backends: **injectable system prompt** (evaluator role;
tunable without changing `prompt_sha256`) + **embedded canonical payload** in the
judge message; token count + send; reply validation; equal-input fields (except
Jev-null fields). HTTP body ≠ Jev wire bytes (ADR-SAG-0019). Prompt text is passed
in by orchestration — no production prose literals in this crate.

## Deliverables

1. **REQ-SAG-0500** — Haiku wire format matches README table (fake transport tests).
2. **REQ-SAG-0501** — Luna wire format + **live** verification of `/v1/responses/input_tokens` (**requested feature 8**) documented in sprint PR evidence.
3. **REQ-SAG-0502** — LLM `INPUT_TOO_LARGE` after full provider message (prompt + payload) is known; invalid question paths refuse send like Jev.
4. **REQ-SAG-0503** — System prompt via `JudgeRequest.llm`; changing prompt changes wire, not `prompt_sha256`.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0503 | Injected prompt in wire; digest unchanged |
| req:REQ-SAG-0500 | Haiku fake-transport wire matches README |
| boundary:jev-backends | Crate root + transport owned here |

## Required validation

```bash
just lint
cargo test -p sc-jev-backends llm -- --nocapture
cargo test -p sc-jev-backends -- test_haiku_request_embeds_canonical_payload --exact --nocapture
cargo test -p sc-jev-backends -- test_luna_request_embeds_canonical_payload --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_system_prompt_is_injected_not_hardcoded --exact --nocapture
cargo test -p sc-jev-backends -- test_prompt_change_does_not_alter_prompt_sha256 --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_input_is_refused_like_jev_and_nothing_is_sent --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_bad_reply_is_typed --exact --nocapture
cargo test -p sc-jev-backends -- test_unknown_question_type_has_no_answer_schema --exact --nocapture
```

Expected: exit 0 on CI (no network). **Feature 8 live probe:** attach Luna
token-count probe output in PR description, or `SKIPPED: no OPENAI_API_KEY` only
with Rand waiver bead (not a CI gate).

## Parity checklist

See **plan.md** parity rows for sprint **a-5** (Haiku/Luna); equal-input record closes in **a-6**.

## This sprint does not close

Sessions, multi-backend CLI, PyO3.
