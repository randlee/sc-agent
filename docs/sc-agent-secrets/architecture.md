# sc-agent-secrets — architecture

**Boundary:** `agent-secrets`  
**Depends on:** `sc-agent-types`

## API

```rust
use sc_agent_types::{BackendKeyId, JevError, RepoRoot};

pub fn resolve_key(id: BackendKeyId, repo_root: &RepoRoot) -> Result<String, JevError>;
```

Callers pass the same git toplevel the facade uses for extractors; this crate
does **not** run `git`. Precedence: **process environment → repo-root `.env` → keyring**. Platform
keyring via `keyring` crate (or project-standard equivalent). Tests use
injectable store; no network.
