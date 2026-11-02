---
id: a-2
phase: a
status: planned
closure_type: boundary
target_boundary: agent-secrets
track: secrets
wave: 2
branch: sprint/a-2-agent-secrets
worktree: sprint/a-2-agent-secrets
recommended_agent: tai
recommended_model: gpt-5.6-terra-medium
must_follow: [a-1]
parallel_safe: [a-3, a-4, a-5]
requirements:
  - REQ-SAG-0101
  - REQ-SAG-0200
  - REQ-SAG-0201
  - REQ-SAG-0202
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0003
owned_paths:
  - "crates/sc-agent-secrets/**"
  - "boundaries/agent-secrets/**"
---

# Sprint a-2 — sc-agent-secrets

## Outcome

Shared credential resolution: **process environment → repo-root `.env` →
keyring** (service = env var name, account = user); injectable for tests;
`KEY_MISSING` matches README semantics.

## Deliverables

1. **REQ-SAG-0200** — `resolve_key(id: BackendKeyId, repo_root: &Path)` for `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` (facade passes git toplevel from a-3; this crate does not run git).
2. **REQ-SAG-0201** — Optional override for `ANTHROPIC_KEYCHAIN_SERVICE` on Anthropic only.
3. **REQ-SAG-0202** — Load vars from repo-root `.env` between env and keyring; `.env` never overrides set process env.
4. **ADR-SAG-0003** — Secrets never logged or included in error messages.

## Explicit code samples

```rust
pub fn resolve_key(id: BackendKeyId, repo_root: &Path) -> Result<String, JevError>;
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:agent-secrets | `resolve_key` injectable; no logging of secret material |
| ADR-SAG-0003 | Errors never echo key values |

## Required validation

```bash
just lint
cargo test -p sc-agent-secrets -- --nocapture
cargo test -p sc-agent-secrets -- test_env_dotenv_keychain_order --exact --nocapture
cargo clippy -p sc-agent-secrets -- -D warnings
```

Expected: exit 0; tests prove precedence env → `.env` → keyring mock; missing key → `KEY_MISSING` without network.

## Parity checklist

`test_api_key_env_first_then_keychain`, `test_jev_key_env_first_then_keychain`, `test_api_key_missing_sends_nothing` (library tests in a-6 consume this crate).

## This sprint does not close

User-level `.env` paths (future).
