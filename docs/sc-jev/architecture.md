# sc-jev — architecture

**Boundary:** `jev`  
**Depends on:** `sc-agent-types`, `sc-agent-http` (r-3), `serde`, `serde_json`, `toml` (API key resolved by **`sc-jev-check`** and passed at `JevClient` construction)

## North star

**`sc-jev` is the only TypeSafe Jev client.** Every Jev HTTP operation the
product needs is implemented here. No other crate sends Jev API traffic.

Phase A lands the **check/judge** HTTP vertical slice. Additional endpoints add
as modules without new top-level client crates.

## Module layout (Phase A)

| Module | Purpose |
| --- | --- |
| `client` | `JevClient<T: Transport = UreqTransport>`: builds the `HttpRequest` (endpoint, `Content-Type`, `Authorization: Bearer` as a `SecretHeader`) and maps the reply |
| `check` | Check/judge wire body (byte-exact payload, pinned model) and response parser |
| `config` | `JevClientConfig`, packaged `jev.toml` (model, host, path, attempts, timeout) |

From r-3, HTTP transport, busy retry, the 8 MiB response cap and redaction
are `sc-agent-http` (ADR-SAG-0026, ADR-SAG-0006 r-3 amendment). This crate is
a wire-format crate: it still owns every TypeSafe Jev API operation
(ADR-SAG-0012). `JevClient::with_default_transport(key)` keeps the r-1
signature and builds `UreqTransport` with a 10 s connect timeout and the
`jev.toml` request timeout. `with_busy_policy` and `with_cancel` replace the
policy and the cancel token; tests use the `sc-agent-http` `test-util`
doubles through the dev-dependency.

## Non-responsibilities

- `check.toml`, extractors, sessions, `run_check` (**`sc-jev-check`**).
- Haiku/Luna (**`sc-jev-backends`**).
- CLI, PyO3.

## Check flow (Phase A)

1. Caller (facade) builds `JudgeRequest` with canonical `PayloadBundle`.
2. `JevClient` as `JudgeBackend` POSTs `req.payload.bytes` byte-exact.
3. Response parsed to `JudgeOutcome`.

Comparison backends are registered and invoked only from **`sc-jev-check`**.
