---
id: a-5
phase: a
status: complete
closure_type: boundary
target_boundary: jev-backends
track: llm
wave: 2
branch: sprint/a-5-backend-llm
worktree: sprint/a-5-backend-llm
recommended_agent: tai
recommended_model: gpt-5.6-terra-medium
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-11, a-4]
requirements:
  - REQ-SAG-0101
  - REQ-SAG-0103
  - REQ-SAG-0500
  - REQ-SAG-0501
  - REQ-SAG-0502
  - REQ-SAG-0504
  - REQ-SAG-0505
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0007
  - ADR-SAG-0012
  - ADR-SAG-0019
  - ADR-SAG-0020
owned_paths:
  - "crates/sc-jev-backends/**"
  - "boundaries/sc-jev-backends/**"
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
2. **REQ-SAG-0501** — Luna wire format + token-count request shape verified with **fake transport** (does **not** close requested feature **8** — see plan resolved decision **4** / bead `agt-luna-live-count`).
3. **REQ-SAG-0502** — LLM `INPUT_TOO_LARGE` after full provider message (prompt + payload) is known; invalid question paths refuse send like Jev.
4. **REQ-SAG-0500 (prompt injection proof)** — System prompt via `JudgeRequest.llm`; changing prompt changes wire, not `prompt_sha256` (REQ-SAG-0503 load/inject owned by **a-6**).
5. **REQ-SAG-0504** — Read Anthropic `stop_reason` / OpenAI `status`; truncation at `max_tokens` → `RESPONSE_INVALID` with reason (tighten **#4**).
6. **REQ-SAG-0505** — Busy retry 429/502/529 + `Retry-After` clamp, same body as Jev (tighten **#5**); HTTP timeouts per ADR-SAG-0007.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:jev-backends | Haiku/Luna fake-transport tests pass (wire, token-count preflight, INPUT_TOO_LARGE, RESPONSE_INVALID, busy retry); key + transport at construction; no `sc-agent-secrets` dep |
| REQ-SAG-0103 | Haiku and Luna `JudgeOutcome` field names match `sc_agent_types::test_util::judge_outcome_field_names()` |
| ADR-SAG-0004 | Pre-send byte/token validation |
| ADR-SAG-0006 | Transport at construction |
| ADR-SAG-0007 | HTTP timeouts on token-count + judge |
| ADR-SAG-0019 | Prompt injection does not alter `prompt_sha256` |

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
cargo test -p sc-jev-backends -- test_luna_token_count_wire_before_judge --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_stop_reason_truncation_is_response_invalid --exact --nocapture
cargo test -p sc-jev-backends -- test_llm_busy_retry_same_body --exact --nocapture
! cargo tree -p sc-jev-backends -i sc-agent-secrets 2>/dev/null
```

Expected: exit 0 on CI (no network). Live `/v1/responses/input_tokens` probe
closes **`agt-luna-live-count`** when a key exists; Phase A does not claim Luna
parity until then.

## Parity checklist

See **plan.md** parity rows for sprint **a-5** (Haiku/Luna); equal-input record closes in **a-6**.

## This sprint does not close

Sessions, multi-backend CLI, PyO3.
