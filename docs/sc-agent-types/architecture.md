# sc-agent-types — architecture

**Boundary:** `agent-types`  
**Depends on:** serde (+ **temporary** `sc-observability-types = "1.5.0"` for v2 shim only)

## Responsibilities

- Single error enum for Jev and CLI (`JevError`).
- JSON envelope helpers aligned with sc-ai-cli.
- **Frozen judge contract:** `JudgeBackend`, `JudgeRequest`, shared domain types.
- **Temporary (release 1–2):** `observability::v2` logging port (see below).

## Domain newtypes (sprint a-1)

```rust
/// Git toplevel for path resolution (from `git rev-parse --show-toplevel`).
pub struct RepoRoot(PathBuf);

/// Git revision or `WORKTREE` for `git show` inputs.
pub struct Rev(String);

pub struct SessionId(uuid::Uuid);

/// Backend credential id (frozen here; sc-agent-secrets consumes it).
pub enum BackendKeyId { TypeSafe, Anthropic, OpenAi }

/// Lowercase hex SHA-256 (64 chars) — canonical payload / equal-input digests.
pub struct Sha256Digest(String);
```

Extractor-facing types (`Subject`, `Span`, `InputSpec`, `Question`, …) and the
**`sc/subjects/1`** document types live in this crate and are frozen in a-1.
Normative JSON rules: [`subjects-schema.md`](subjects-schema.md).

```rust
pub struct SubjectsSource { pub path: String, pub rev: String, pub blob_sha: String }
pub struct SubjectsProducer {
    pub name: String,
    pub version: String,
    pub language: String,
    pub grammar: String,
    pub grammar_version: String,
    pub query_sha256: Sha256Digest,
    pub mode: String, // "matches" for tree-sitter
}
pub struct Span {
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}
pub struct Subject {
    pub id: String,
    pub capture: String,
    pub text: String,
    pub span: Span,
}
pub struct SubjectsDocument {
    pub schema: String, // "sc/subjects/1"
    pub source: SubjectsSource,
    pub producer: SubjectsProducer,
    pub subjects: Vec<Subject>,
}
```

## Judge contract (sprint a-1)

```rust
pub struct PayloadBundle {
    pub bytes: Vec<u8>,
    pub sha256: Sha256Digest,
}

pub struct LlmWireContext {
    pub system_prompt: String,
}

pub struct JudgeRequest {
    pub payload: PayloadBundle,
    pub llm: Option<LlmWireContext>,
}

/// Normalized TypeSafe Jev answer JSON both backends emit (README parity).
pub struct JudgeOutcome {
    pub answers: serde_json::Value,
    pub backend: BackendId,
    /// Jev probability fields when present; `None` for LLM backends.
    pub probabilities: Option<serde_json::Value>,
    /// Provider metadata (stop_reason, status, model id) for debugging; not equal-input.
    pub provider_meta: Option<serde_json::Value>,
}

pub struct EqualInputRecord {
    pub prompt_sha256: Sha256Digest,
    pub leak: bool,
    pub commands: Vec<String>,
    pub equal: bool,
    /// Copy of subjects `producer` block when check used subjects/command inputs.
    pub producer: Option<SubjectsProducer>,
}

pub enum BackendId { Jev, AnthropicHaiku, OpenAiLuna /* … */ }

pub trait JudgeBackend: Send + Sync {
    fn id(&self) -> BackendId;
    fn validate_for_send(&self, req: &JudgeRequest) -> Result<(), JevError>;
    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError>;
}
```

The facade builds one `PayloadBundle` and sets `llm` for Haiku/Luna only
(ADR-SAG-0019). Implementations hold HTTP transport and **resolved API key** at
**construction**; the trait does not name transport types or accept secrets per call.

Contract tests + `MockJudgeBackend` live in **`sc-agent-types`** (`test-util`).
A shared test asserts `MockJudgeBackend`, `sc-jev`, and `sc-jev-backends` fill the
same `JudgeOutcome` fields for a fixture request.

## Temporary v2 logging shim (`observability::v2`)

```rust
pub struct LogEvent {
    pub session_id: uuid::Uuid,
    pub backend: BackendId,
    pub prompt_sha256: Sha256Digest,
    pub operation: String,
    // no secrets
}

pub trait SessionLog: Send + Sync {
    fn try_log(&self, event: LogEvent) -> Result<(), EventError>;
}
pub type SharedSessionLog = Arc<dyn SessionLog>;
```

- Depends only on **`sc-observability-types`** (not full `sc-observability`).
- **`sc-ai`** and **`sc-jev-py`** build `sc_observability::v2::Logger` and adapt to `SharedSessionLog`.
- **`sc-jev-check`** takes `SharedSessionLog` by injection (not `sc-jev`).

**Removal (bead `agt-obs-v2-shim-removal`):** delete shim when upstream v2 ships.

## Non-responsibilities

- HTTP, keyring, check.toml parsing, CLI argv, building the real Logger.
