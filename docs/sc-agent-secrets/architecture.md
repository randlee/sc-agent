# sc-agent-secrets — architecture

**Boundary:** `agent-secrets`  
**Depends on:** `sc-agent-types`

## API

```rust
use sc_agent_types::{BackendKeyId, JevError, RepoRoot};

pub fn resolve_key(id: BackendKeyId, repo_root: &RepoRoot) -> Result<String, JevError>;

pub fn resolve_key_with(
    id: BackendKeyId,
    repo_root: &RepoRoot,
    env: &impl EnvSource,
    keyring: &impl KeyringSource,
    account: impl AsRef<str>,
) -> Result<String, JevError>;
```

Callers pass the same git toplevel the facade uses for extractors; this crate
does **not** run `git`. Precedence: **process environment → repo-root `.env` → keyring**.
`resolve_key` uses the process environment, the OS user as the keyring account,
and the platform keyring (`keyring` crate: Keychain, Credential Manager, or
Secret Service). `resolve_key_with` is the injectable entry tests use so they
never touch the network or the real keyring. The keyring service is the
environment variable name. Anthropic lookups honor `ANTHROPIC_KEYCHAIN_SERVICE`.
Errors never include secret material.
