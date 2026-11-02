# sc-jev — architecture

**Boundary:** `jev`  
**Depends on:** `sc-agent-types`, `sc-agent-secrets`, `sc-jev-backends` (LLM judges only)

## North star

**`sc-jev` is the only TypeSafe Jev client.** Every Jev HTTP operation the
product needs is implemented here (typed requests, responses, errors, transport).
No other crate sends Jev API traffic. Callers (orchestration, plan-anchor,
`sc-ai`, Python) use `sc-jev` only.

Phase A lands the **choice-check / judge** vertical slice (Python `packages/jev-check`
parity). Additional Jev endpoints add as modules in later sprints without new
top-level client crates.

## Module layout (intended)

| Module | Purpose | Phase A |
| --- | --- | --- |
| `api` / `client` | TypeSafe Jev HTTP transport, auth, retries, future endpoints | Check/judge path |
| `check` | `check.toml`, extractors, payload build, `run_check`, sessions | Yes |
| `extract::delimited_regions` | Marker pairs in source (ADR-SAG-0015) | Phase A |
| `payload` | Canonical bytes, digests, size limits | Yes |

Comparison backends (Haiku, Luna) stay in **`sc-jev-backends`**; `sc-jev::check`
invokes them via `JudgeBackend` when selected.

## Check flow (Phase A)

1. Load `check.toml` + packaged `jev.toml` defaults.
2. Run extractors at git repo root → question map + state JSON.
3. Build `PayloadBundle` (canonical bytes).
4. Jev path: `sc-jev` API client (one request, byte-exact).
5. Optional LLM paths: `sc-jev-backends` with equal-input record.
6. Session files for LLM backends; `ask` on session.

## Registration

LLM backends register by `BackendId`; local judge adds a backend impl later
without changing the public `sc-jev` entry points for check.
