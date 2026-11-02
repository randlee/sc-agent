# sc-jev-backends — architecture

**Boundary:** `jev-backends`  
**Depends on:** `sc-agent-types` only (API key resolved by **`sc-jev-check`** and passed at backend construction)

**HTTP policy (Phase A):** connect timeout 10s, overall request timeout 120s on
provider calls; busy retry 429/502/529 with same body (REQ-SAG-0505). Test builders
may shorten timeouts.

Non-Jev **comparison** backends only. TypeSafe Jev HTTP lives in **`sc-jev`**.

## Modules

| Module | Backend |
| --- | --- |
| `anthropic` | Haiku (API-direct) |
| `openai` | Luna (API-direct) |
| `transport` | Shared HTTP trait for LLM calls (may share patterns with `sc-jev`) |

Each implements `JudgeBackend` from `sc-agent-types`.

## LLM wire envelope

Backends accept **caller-supplied** evaluator system prompt text plus canonical
payload bytes (ADR-SAG-0019). This crate does not read `check.toml` and does not
ship production prompt prose in source — only wiring, validation, and HTTP.

## Transport

```rust
pub trait BackendTransport: Send + Sync {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, JevError>;
}
```

No CLI, no PyO3, no `check.toml`, no Jev API endpoints.
