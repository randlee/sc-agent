---
id: a-4
phase: a
status: planned
closure_type: boundary
target_boundary: jev
track: jev
wave: 2
branch: sprint/a-4-jev-api-check
worktree: sprint/a-4-jev-api-check
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-3]
parallel_safe: [a-5]
requirements:
  - REQ-SAG-0110
  - REQ-SAG-0400
  - REQ-SAG-0401
adrs:
  - ADR-SAG-0006
  - ADR-SAG-0012
owned_paths:
  - "crates/sc-jev/src/api/**"
  - "crates/sc-jev/src/client/**"
  - "boundaries/jev/**"
---

# Sprint a-4 — TypeSafe Jev API (check endpoint)

## Outcome

Jev HTTP client lives in **`sc-jev`** (foundation for **100% of Jev API**).
Phase A implements the **check/judge** request path: byte-exact body with
`model` key, busy retry policy, strict answer parsing (tighten **#3**
probabilities). No Jev traffic in other crates.

## Deliverables

1. **REQ-SAG-0110** — `sc-jev` owns TypeSafe Jev HTTP; this sprint establishes `client` + check/judge method(s).
2. **REQ-SAG-0400** — Injectable transport; fake transport in tests.
3. **REQ-SAG-0401** — Retry only 429/502/529 with `Retry-After` clamp; same body each attempt.
4. **ADR-SAG-0012** — All future Jev endpoints extend `sc-jev::api`, not new crates.

## Explicit code samples

```rust
// crates/sc-jev/src/client/mod.rs
pub struct JevClient { /* auth, base url, transport */ }

impl JevClient {
    pub fn judge_check(&self, body: &[u8]) -> Result<JudgeResponse, JevError>;
}
```

## Acceptance criteria

```bash
just lint
cargo test -p sc-jev jev_api -- --nocapture
cargo test -p sc-jev -- test_one_check_is_one_request_byte_exact --exact --nocapture
cargo test -p sc-jev -- test_busy_is_retried_with_the_same_body --exact --nocapture
cargo clippy -p sc-jev -- -D warnings
! rg -l 'typesafe' crates/sc-jev-backends/src/ 2>/dev/null | head -1 | grep -q . || true
```

Expected: exit 0; Jev parity transport tests run against `sc-jev`; no TypeSafe
module under `sc-jev-backends`.

## Parity checklist

All Jev-backend rows in README checklist.

## This sprint does not close

Other Jev API endpoints (REQ-SAG-0110 remainder), Haiku/Luna, sessions, CLI.
