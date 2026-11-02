# sc-agent-secrets — architecture

**Boundary:** `agent-secrets`  
**Depends on:** `sc-agent-types`

## API

```rust
pub enum BackendKeyId { Typesafe, Anthropic, OpenAi }

pub fn resolve_key(id: BackendKeyId) -> Result<String, JevError>;
```

Precedence: **process environment → repo-root `.env` → keyring**. Platform
keyring via `keyring` crate (or project-standard equivalent). Tests use
injectable store; no network.
