# sc-jev-backends — architecture

**Boundary:** `jev-backends`  
**Depends on:** `sc-agent-types`, `sc-agent-secrets`

Non-Jev **comparison** backends only. TypeSafe Jev HTTP lives in **`sc-jev`**.

## Modules

| Module | Backend |
| --- | --- |
| `anthropic` | Haiku (API-direct) |
| `openai` | Luna (API-direct) |
| `transport` | Shared HTTP trait for LLM calls (may share patterns with `sc-jev`) |

Each implements `JudgeBackend` from `sc-agent-types`.

## Transport

```rust
pub trait BackendTransport: Send + Sync {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, JevError>;
}
```

No CLI, no PyO3, no `check.toml`, no Jev API endpoints.
