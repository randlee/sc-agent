---
id: a-4
phase: a
status: complete
closure_type: boundary
target_boundary: jev
track: jev
wave: 2
branch: sprint/a-4-backend-jev
worktree: sprint/a-4-backend-jev
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-11, a-5]
requirements:
  - REQ-SAG-0101
  - REQ-SAG-0103
  - REQ-SAG-0110
  - REQ-SAG-0400
  - REQ-SAG-0401
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0006
  - ADR-SAG-0012
  - ADR-SAG-0013
  - ADR-SAG-0019
  - ADR-SAG-0020
owned_paths:
  - "crates/sc-jev/src/config.rs"
  - "crates/sc-jev/src/client/**"
  - "crates/sc-jev/Cargo.toml"
  - "crates/sc-jev/src/lib.rs"
  - "boundaries/sc-jev/**"
---

# Sprint a-4 — TypeSafe Jev API (check endpoint)

## Outcome

Jev HTTP client lives in **`sc-jev`** (foundation for **100% of Jev API**).
Phase A implements check/judge HTTP plus **`JevClient: JudgeBackend`**. No
`check.toml`, no extractors, no dependency on `sc-jev-backends`.

## Deliverables

1. **REQ-SAG-0110** — `sc-jev` owns TypeSafe Jev HTTP; `client` + check/judge path.
2. **REQ-SAG-0400** — Injectable transport at `JevClient` construction; byte-exact body.
3. **REQ-SAG-0401** — Busy retry 429/502/529 + `Retry-After`.
4. **ADR-SAG-0012** — Future Jev endpoints extend `sc-jev::api`.
5. **Vendor pins (tighten #12)** — TypeSafe API version / model defaults in client config (committed in this crate).

## Explicit code samples

```rust
pub struct JevClient { /* transport injected at new() */ }

impl JudgeBackend for JevClient {
    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        let _ = req.llm; // Jev ignores LLM system prompt
        // POST canonical req.payload.bytes
    }
}
```

## Must-follow

Consumes **`JudgeBackend` / `JudgeRequest` from a-1** only (not a-3 payload builder).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:jev | `JevClient: JudgeBackend`; transport + resolved key at construction |
| ADR-SAG-0004 | `validate_for_send` + refuse before send on oversize input |
| ADR-SAG-0006 | No transport parameter on `judge` |
| REQ-SAG-0103 | `JevClient` `JudgeOutcome` field names match `sc_agent_types::test_util::judge_outcome_field_names()` |

## Required validation

```bash
just lint
cargo test -p sc-jev jev_api -- --nocapture
cargo test -p sc-jev -- test_one_check_is_one_request_byte_exact --exact --nocapture
cargo test -p sc-jev -- test_busy_is_retried_with_the_same_body --exact --nocapture
cargo clippy -p sc-jev -- -D warnings
test -z "$(rg -l -i typesafe crates/sc-jev-backends/src 2>/dev/null || true)"
```

Expected: exit 0; last command fails if TypeSafe module appears under backends.

## This sprint does not close

Other Jev API endpoints, Haiku/Luna, facade `run_check`, CLI.
