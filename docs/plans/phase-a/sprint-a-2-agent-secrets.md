---
id: a-2
phase: a
status: complete
closure_type: boundary
target_boundary: agent-secrets
track: secrets
wave: 2
branch: sprint/a-2-agent-secrets
worktree: sprint/a-2-agent-secrets
recommended_agent: tai
recommended_model: gpt-5.6-terra-medium
must_follow: [a-1]
parallel_safe: [a-3, a-11, a-4, a-5]
requirements:
  - REQ-SAG-0101
  - REQ-SAG-0200
  - REQ-SAG-0201
  - REQ-SAG-0202
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0003
  - ADR-SAG-0013
  - ADR-SAG-0020
owned_paths:
  - "crates/sc-agent-secrets/**"
  - "boundaries/sc-agent-secrets/**"
---

# Sprint a-2 — sc-agent-secrets

## Outcome

Shared credential resolution: **process environment → repo-root `.env` →
keyring** (service = env var name, account = user); injectable for tests;
`KEY_MISSING` matches README semantics.

## Deliverables

1. **REQ-SAG-0200** — `resolve_key(id: BackendKeyId, repo_root: &RepoRoot) -> Result<ResolvedApiKey, JevError>` for `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` (facade passes git toplevel from a-3; this crate does not run git).
2. **REQ-SAG-0201** — Optional override for `ANTHROPIC_KEYCHAIN_SERVICE` on Anthropic only.
3. **REQ-SAG-0202** — Load vars from repo-root `.env` between env and keyring; `.env` never overrides set process env.
4. **NFR-SAG-0002** / **ADR-SAG-0013** — Secrets never appear in CLI output or logs. This crate emits no log lines. **ADR-SAG-0003** is the resolution order (process env → repo-root `.env` → keyring), not the session-log policy. Credential errors are `JevError` ids (**ADR-SAG-0001**) and never include secret material.
5. **REQ-SAG-0200** / **NFR-SAG-0002** / **ADR-SAG-0020** — `EnvSource` and `KeyringSource` are the open injection traits. Feature `test-util` exposes `MapEnv` and `MapKeyring`. *Superseded by ADR-SAG-0029 (a): no `test-util` feature; `MapEnv` and `MapKeyring` are `cfg(test)` crate-private.* `resolve_key` returns `ResolvedApiKey`, whose `Debug` and `Display` redact the secret.

## Explicit code samples

```rust
pub fn resolve_key(id: BackendKeyId, repo_root: &RepoRoot) -> Result<ResolvedApiKey, JevError>;
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:agent-secrets | `resolve_key` injectable; returns `ResolvedApiKey`; no logging of secret material |
| ADR-SAG-0001 | Credential failures use `JevError` codes `KEY_MISSING` and `CONFIG_INVALID` |
| ADR-SAG-0003 | Resolution order is process env → `.env` → keyring; errors never echo key values |
| NFR-SAG-0002 | `fix_hint`, `Display`, and `Debug` contain no secret material (log lines: ADR-SAG-0013) |
| ADR-SAG-0020 | Injection traits and the `test-util` doubles are the recorded extension points. *Superseded by ADR-SAG-0029 (a): no `test-util` feature; `MapEnv` and `MapKeyring` are `cfg(test)` crate-private.* |

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
