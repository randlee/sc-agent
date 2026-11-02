# sc-agent-types — architecture

**Boundary:** `agent-types`  
**Depends on:** serde (+ **temporary** `sc-observability-types = "1.5.0"` for v2 shim only)

## Responsibilities

- Single error enum for Jev and CLI (`JevError`).
- JSON envelope helpers aligned with sc-ai-cli.
- `JudgeBackend` trait definition — implementations live in other crates.
- **Temporary (release 1–2):** `observability::v2` logging port (see below).

## Temporary v2 logging shim (`observability::v2`)

Until **sc-observability v2** is the stable released API, this crate holds a small
**SessionLog** port (same shape as sc-runtime’s injectable v2 logger), so we do
**not** publish a separate observability crate on crates.io:

```rust
pub mod observability {
    pub mod v2 {
        pub trait SessionLog: Send + Sync { /* emit LogEvent → EventError */ }
        pub type SharedSessionLog = Arc<dyn SessionLog>;
        // NoopSessionLog, test-util doubles
    }
}
```

- Depends only on **`sc-observability-types`** (not full `sc-observability`).
- **`sc-ai`** builds `sc_observability::v2::Logger` and adapts to `SharedSessionLog`.
- **`sc-jev`** takes `SharedSessionLog` by injection.

**Removal plan (bead `agt-obs-v2-shim-removal`):** delete `observability::v2`,
drop `sc-observability-types` from this crate if upstream exposes the port, migrate
call sites to the released interface.

## Non-responsibilities

- HTTP, keyring, check.toml parsing, CLI argv, building the real Logger.

## Key types

```rust
pub trait JudgeBackend: Send + Sync { /* see phase plan */ }
pub enum JevError { /* one variant per README error */ }
pub struct PayloadBundle { pub bytes: Vec<u8>, pub sha256: String }
```
