# sc-jev — architecture

**Boundary:** `jev`  
**Depends on:** `sc-agent-types`, `sc-agent-secrets` (for auth key resolution at call site)

## North star

**`sc-jev` is the only TypeSafe Jev client.** Every Jev HTTP operation the
product needs is implemented here. No other crate sends Jev API traffic.

Phase A lands the **check/judge** HTTP vertical slice. Additional endpoints add
as modules without new top-level client crates.

## Module layout (Phase A)

| Module | Purpose |
| --- | --- |
| `api` / `client` | TypeSafe transport, auth, retries |
| `check` | Check/judge request encoding (HTTP only) |

## Non-responsibilities

- `check.toml`, extractors, sessions, `run_check` (**`sc-jev-check`**).
- Haiku/Luna (**`sc-jev-backends`**).
- CLI, PyO3.

## Check flow (Phase A)

1. Caller (facade) builds `JudgeRequest` with canonical `PayloadBundle`.
2. `JevClient` as `JudgeBackend` POSTs `req.payload.bytes` byte-exact.
3. Response parsed to `JudgeOutcome`.

Comparison backends are registered and invoked only from **`sc-jev-check`**.
