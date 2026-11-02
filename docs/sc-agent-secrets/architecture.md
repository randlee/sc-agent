# sc-agent-secrets — architecture

**Boundary:** `agent-secrets`  
**Depends on:** `sc-agent-types`

## API

```rust
pub enum BackendKeyId { Typesafe, Anthropic, OpenAi }

pub fn resolve_key(id: BackendKeyId, repo_root: &Path) -> Result<String, JevError>;
```

Callers pass the same git toplevel the facade uses for extractors; this crate
does **not** run `git`. Precedence: **process environment → repo-root `.env` → keyring**. Platform
keyring via `keyring` crate (or project-standard equivalent). Tests use
injectable store; no network.
