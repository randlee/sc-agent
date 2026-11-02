# sc-agent-secrets — architecture

**Boundary:** `agent-secrets`  
**Depends on:** `sc-agent-types`

## API

```rust
use sc_agent_types::{BackendKeyId, JevError, RepoRoot};

pub fn resolve_key(id: BackendKeyId, repo_root: &RepoRoot) -> Result<ResolvedApiKey, JevError>;

pub fn resolve_key_with(
    id: BackendKeyId,
    repo_root: &RepoRoot,
    env: &impl EnvSource,
    keyring: &impl KeyringSource,
    account: impl AsRef<str>,
) -> Result<ResolvedApiKey, JevError>;
```

Callers pass the same git toplevel the facade uses for extractors; this crate
does **not** run `git`. Precedence: **process environment → repo-root `.env` →
keyring** (ADR-SAG-0003). `resolve_key` uses the process environment, the OS
user as the keyring account, and the platform keyring (`keyring` crate:
Keychain, Credential Manager, or Secret Service). `resolve_key_with` is the
injectable entry. `EnvSource` and `KeyringSource` are the open injection
traits. The `test-util` feature exposes `MapEnv` and `MapKeyring`
(ADR-SAG-0020).

The keyring service is the environment variable name. Anthropic lookups honor
`ANTHROPIC_KEYCHAIN_SERVICE`.

`ResolvedApiKey`'s `Debug` and `Display` redact the secret. `as_str` is the
only read. This crate emits no log lines (NFR-SAG-0002, ADR-SAG-0013).
Unreadable `.env` or keyring failures are `CONFIG_INVALID` with a sanitized
cause and a recovery hint, never secret bytes. A miss is `KEY_MISSING`
(ADR-SAG-0001).
