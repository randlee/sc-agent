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
  - REQ-SAG-0504
  - REQ-SAG-0505
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
2. **REQ-SAG-0501** — Luna wire format + token-count request shape verified with **fake transport** (feature **8**). Optional: maintainer with `OPENAI_API_KEY` attaches live `/v1/responses/input_tokens` output in PR — **not** required for phase close (Rand has no OpenAI key).
3. **REQ-SAG-0502** — LLM `INPUT_TOO_LARGE` after full provider message (prompt + payload) is known; invalid question paths refuse send like Jev.
4. **REQ-SAG-0503** — System prompt via `JudgeRequest.llm`; changing prompt changes wire, not `prompt_sha256`.
5. **REQ-SAG-0504** — Read Anthropic `stop_reason` / OpenAI `status`; truncation at `max_tokens` → `RESPONSE_INVALID` with reason (tighten **#4**).
6. **REQ-SAG-0505** — Busy retry 429/502/529 + `Retry-After` clamp, same as Jev (tighten **#5**).

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

Expected: exit 0 on CI (no network). Live Luna probe is **optional** evidence
when a key exists; phase close does **not** require one.

## Parity checklist

See **plan.md** parity rows for sprint **a-5** (Haiku/Luna); equal-input record closes in **a-6**.

## This sprint does not close

Sessions, multi-backend CLI, PyO3.
