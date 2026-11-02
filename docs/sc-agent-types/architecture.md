# sc-agent-types — architecture

**Boundary:** `agent-types`  
**Depends on:** serde (+ **temporary** `sc-observability-types = "1.5.0"` for v2 shim only)

## Responsibilities

- Single error enum for Jev and CLI (`JevError`).
- JSON envelope helpers aligned with sc-ai-cli.
- **Frozen judge contract:** `JudgeBackend`, `JudgeRequest`, shared domain types.
- **Temporary (release 1–2):** `observability::v2` logging port (see below).

## Judge contract (sprint a-1)

```rust
pub struct PayloadBundle {
    pub bytes: Vec<u8>,
    pub sha256: String,
}

pub struct LlmWireContext {
    pub system_prompt: String,
}

pub struct JudgeRequest {
    pub payload: PayloadBundle,
    pub llm: Option<LlmWireContext>,
}

pub struct JudgeOutcome { /* normalized answers + backend metadata */ }

pub struct EqualInputRecord {
    pub prompt_sha256: String,
    pub leak: bool,
    pub commands: Vec<String>,
    pub equal: bool,
}

pub enum BackendId { Jev, AnthropicHaiku, OpenAiLuna /* … */ }

/// Question map keyed for canonical JSON (sorted keys at serialize time).
pub type QuestionMap = BTreeMap<String, Question>;

pub trait JudgeBackend: Send + Sync {
    fn id(&self) -> BackendId;
    fn validate_for_send(&self, req: &JudgeRequest) -> Result<(), JevError>;
    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError>;
}
```

The facade builds one `PayloadBundle` and sets `llm` for Haiku/Luna only
(ADR-SAG-0019). Implementations hold HTTP transport at **construction**; the
trait does not name transport types.

Contract tests + `MockJudgeBackend` live in **`sc-agent-types`** (`test-util`).

## Temporary v2 logging shim (`observability::v2`)

```rust
pub struct LogEvent {
    pub session_id: uuid::Uuid,
    pub backend: BackendId,
    pub prompt_sha256: String,
    pub operation: String,
    // no secrets
}

pub trait SessionLog: Send + Sync {
    /// Best-effort append; never panics; errors are logged to stderr in adapters only.
    fn try_log(&self, event: LogEvent) -> Result<(), EventError>;
}
pub type SharedSessionLog = Arc<dyn SessionLog>;
```

- Depends only on **`sc-observability-types`** (not full `sc-observability`).
- **`sc-ai`** builds `sc_observability::v2::Logger` and adapts to `SharedSessionLog`.
- **`sc-jev-check`** takes `SharedSessionLog` by injection (not `sc-jev`).

**Removal (bead `agt-obs-v2-shim-removal`):** delete shim when upstream v2 ships.

## Non-responsibilities

- HTTP, keyring, check.toml parsing, CLI argv, building the real Logger.
