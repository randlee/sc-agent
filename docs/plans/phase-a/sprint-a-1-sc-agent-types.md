<!-- Generated from bead agt-4p2.1 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-1: sc-agent-types
- id: `a-1` (bead `agt-4p2.1`)
- branch: `sprint/a-1-sc-agent-types`
- depends on: none
- copy script: `docs/plans/phase-a/copy/a-1.sh`

## Description

Build sc-agent-types. Governing records: REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0006, REQ-SAG-0101..0104. Code to copy from sc-agent-ref: see design.

## Design

```yaml
sprint:
  id: a-1
  closure_type: contract
  target_boundary: sc-agent-types
  owned_paths: [crates/sc-agent-types/**, Cargo.toml, clippy.toml]   # Cargo.toml and clippy.toml = the workspace root files
  must_follow: []
  requirements: [REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0006, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0013, REQ-SAG-0101, REQ-SAG-0102, REQ-SAG-0103, REQ-SAG-0104, REQ-SAG-0401, REQ-SAG-0601]
  adrs: [ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0101 end to end (decide --model <short name> resolves through the catalog built from the global config): a-10"
    - "REQ-SAG-0102 'add a model by editing JSON, no rebuild': config part a-4 (AgentConfig::catalog), end to end a-10"
    - "REQ-SAG-0104 AC1 (a token cancelled by the caller stops an operation in another crate): a-9 (sc-decision-runner cancel tests, token created in the test crate); a-1 keeps the in-crate CancelToken tests"
    - "REQ-SAG-0104 AC2 (a CancelToken cancelled from Python stops a request): a-10, sc-agent-py"
    - "REQ-SAG-0001 AC2 (workspace-wide search that no crate generates a UUID): a-10, the last sprint, once the workspace is complete; a-1 only rejects a UUID in SessionId::from_str"
    - "REQ-SAG-0601 cancel ends a runner holdoff at once: a-9 (uses CancelToken::wait_until/on_cancel from here)"
    - "REQ-SAG-0010 / ApiKey::as_str rule: a-1 writes both clippy.toml entries; each crate's clippy run enforces them; the single allowed as_str site is a-5's ureq_transport"
```

# Sprint A-1: sc-agent-types (bead agt-4p2.1)

Copy script: `docs/plans/phase-a/copy/a-1.sh` (sources `lib.sh`; source sc-agent-ref@dce239a). Dry run 2026-11-09 (reconciliation): 18 copy steps, 10 files, 820 lines.
Target: `crates/sc-agent-types` plus the workspace root (`Cargo.toml`, `clippy.toml`).
Governing: REQ-SAG-0001, 0002, 0006, 0009, 0010, 0101, 0102, 0103, 0104; ADR-SAG-0004 (bottom layer; the shared `ErrorId`/`ErrorDetail` convention); REQ-SAG-0401 (no env-var names here); REQ-SAG-0601 (cancel ends a holdoff wait at once).
Owner decisions: owner-short-names, owner-model-catalog, owner-provider-and-model, owner-openai-naming, owner-cancel-token, owner-no-consumer-types, owner-error-contract, owner-errors-settled, owner-ids-ulid, owner-no-sleep, owner-lowlevel-no-tokio, owner-architecture.

Layering (ADR-SAG-0004, every bead uses it): sc-agent-types <- sc-agent-config, sc-agent-secrets, sc-agent-log <- sc-agent-http <- sc-decision-schema <- sc-decision-systemone, sc-decision-local, sc-decision-llm (sc-decision-llm also uses sc-llm-provider, which sits on sc-agent-http and needs no decision types) <- sc-decision-runner <- decide (bin and lib) <- sc-agent-py (Python API, A-10). sc-treesitter stands alone.

Decisions in this design:
- `Provider` is a closed enum of six (TypeSafe, Anthropic, OpenAi, OpenRouter, Nous, Together), not `#[non_exhaustive]`, so sc-agent-secrets matches it exhaustively.
- Local decision runners are global-config entries (A-4 `local_runners`), selected by name in decide (A-10); `ShortName` has exactly the nine.
- Catalog = built-in rows + global-config rows; per line the newest release date wins. On an equal release date the global-config model wins (assistant choice; owner: IDC, decide doctor warns, REQ-SAG-0503). sc-agent-types reads no files.
- Built-in rows carry each model's published release date (REQ-SAG-0102; research/model-dates.md, checked 2026-11-09): jev-1.13.0 2026-09-15 (date supplied by the owner 2026-11-09; docs.typesafe.ai publishes none), jev-1.14.0 2026-10-14, claude-haiku-5-5 2026-10-07, claude-sonnet-5-5 2026-09-28, claude-opus-5-5 2026-09-22, claude-fable-5-2 2026-11-09, gpt-6-luna 2026-09-22, gpt-6.1-terra 2026-10-26, gpt-6.1-sol 2026-09-29, gpt-6-astra 2026-09-03. Ids are provider-neutral (REQ-SAG-0103); each provider maps them to its own id in sc-llm-provider (A-7), e.g. OpenRouter `anthropic/claude-haiku-5.5`.
- Provider ranking (REQ-SAG-0103, assistant decision): `Provider::PRIORITY` = TypeSafe, Anthropic, OpenAi, OpenRouter, Together, Nous. `Provider::ranking(line)` is the providers that serve a line, in that order: jev -> [TypeSafe]; haiku, sonnet, opus, fable -> [Anthropic, OpenRouter, Together]; luna, terra, sol, astra -> [OpenAi, OpenRouter, Together]; an id on no line (`None`) -> [OpenRouter, Together, Nous]. decide takes the first with a detected key; `--provider` is only an optional override (A-10).
- `ModelChoice::Version(id)` resolves to itself without a catalog check (REQ-SAG-0101 AC2; REQ-SAG-0502/0503 allow unlisted models).
- `SessionId` (ULID) lives here: sc-agent-log, sc-decision-schema and the runner all use it.
- `CancelToken` has `wait_until(deadline)` and `on_cancel` registration; no sleep. The runner's queue registers a hook so held-off jobs and idle workers wake at once on cancel (A-9).
- Every crate's error enum implements `ErrorId` (below). Ids are `"<crate-prefix>.<variant_snake>"`. This replaces every SCREAMING_SNAKE code, `code()`/`lower_code()`/`lower_id()` method and serde-tag id scheme. No crate uses `thiserror`.

Size: about 420 lines of code and 200 lines of tests.

## Shared files (a-1 owns them)

a-1 owns the workspace root `Cargo.toml` (`[workspace] members`, `[workspace.package]`, `[workspace.lints]`) and `clippy.toml` (N3). They are consumed by every later sprint; a-3, a-4 and a-8 start from them (must_follow a-1, artifact = workspace root `Cargo.toml` + `clippy.toml`, trigger dev-push: a-1 pushes them right after its copy commit). No other sprint edits either file: a new crate only adds its directory under `crates/`, which `members = ["crates/*"]` picks up. `Cargo.lock` is generated by cargo and owned by no sprint (each merge regenerates it).

`clippy.toml` holds the `sc_agent_secrets::ApiKey::as_str` disallowed-methods entry from the first commit that writes it, so a-5 never edits the file. Checked with clippy 0.1.99 (2026-09-28): an entry whose crate is not a dependency of the crate being linted is skipped silently (no warning), so the entry is harmless in sc-agent-types and every crate that does not depend on sc-agent-secrets; an entry whose crate resolves but whose item does not gives a warning that `-D warnings` does not turn into an error. In crates that depend on sc-agent-secrets (and in sc-agent-secrets itself) the entry resolves and every call is an error unless the call site carries `#[expect(clippy::disallowed_methods, reason = ..)]`.

## Required validation (run at each push; the acceptance list is the one acceptance list)

- `cargo build -p sc-agent-types`
- `cargo clippy --workspace --all-targets -- -D warnings` (workspace denies from N3)
- `cargo test -p sc-agent-types`
- `cargo tree -p sc-agent-types -e normal`: only serde and ulid (and their deps); no tokio, no sc-agent crate
- `rg -n 'sleep' crates/sc-agent-types/src` finds no sleep call; `rg -n '_API_KEY' crates/sc-agent-types` finds nothing

## 1. Copy list (a-1.sh)

| Source (sc-agent-ref) | Lines | Target | What |
|---|---|---|---|
| crates/sc-agent-types/LICENSE | all | LICENSE | MIT |
| src/cancel.rs | 1-54 | src/cancel.rs | CancelToken (flag, clone shares, ptr Eq) |
| src/models.rs | 11-35 | src/model.rs | imports + UnknownModel (becomes ModelId) |
| src/models.rs | 144-180 | src/model.rs | Display, FromStr, serde for ModelId |
| src/models.rs | 198-304 | src/model.rs | ModelDescriptor, MODELS, row() |
| src/models.rs | 401-473 | src/catalog.rs | ModelAliases struct, builtin, alias_of, known_models, latest |
| src/models.rs | 508-548 | src/catalog.rs | with_file_model |
| src/models.rs | 623-646 | src/release_date.rs | check_date |
| src/ids.rs | 261-337 | src/provider.rs | BackendKeyId (enum, ALL, as_str, Display, FromStr, serde) |
| src/ids.rs | 261-337 | src/short_name.rs | same shape, for ShortName |
| src/ids.rs | 180-259, 1369-1377 | src/session_id.rs | SessionId + strict parse_ulid |
| tests/models.rs | 1-32, 52-123, 149-171, 191-208 | tests/catalog.rs | round-trip, newest-wins, two-lines, latest lookup, jev latest |
| tests/ids.rs | 67-85, 252-282 | tests/names.rs | documented spelling, SessionId is ULID |

## 2. Required edits after copying

- cancel.rs: rewrite onto `Arc<Inner>` (section 3). Doc header: drop "a run and its backends". `cancel()` sets the flag and takes the hook list while holding `state`, `notify_all`s, releases the lock, then calls each hook (no hook runs under the lock). Recover a poisoned lock with `PoisonError::into_inner` (no panic).
- model.rs: `UnknownModel` becomes `ModelId(Cow<'static, str>)` with a private field and `pub(crate) const fn from_static`. Delete the variant enum and `known()` (models.rs:51-105), `family()`/`ModelFamily` (126-142, 182-196), `ModelDescriptor::model()` with its `expect` (214-225), the family column and the `Other` arms. FromStr error: `TypesError::InvalidModelId`. Row doc (247-253): drop "only jev, haiku and luna are selectable" and the freshness-test reference. `row()` takes `(ModelId, ShortName, ReleaseDate)` consts.
- catalog.rs: `ModelAliases` -> `Catalog` with fields keyed by `ShortName`/`ModelId`. Delete `Entry.source`, `ModelSource`, `ResolvedModel`, `source()`, `resolved()`, `with_override`, `overlay_overrides`, `check_family`, `aliases()`, `known_models()`. `alias_of` -> `line_of`. `with_file_model` -> `with_config_model(ModelId, ShortName, ReleaseDate)`: delete the parse, check_date and check_family calls, the unknown-alias branch and the `Override` test (models.rs:526). `row.model()` -> `row.model.clone()`.
- release_date.rs: `check_date` -> `impl FromStr for ReleaseDate`; `bytes[4]`/`bytes[7]` -> `bytes.get(4) == Some(&b'-')`; store `{year: u16, month: u8, day: u8}` (derived `Ord` compares dates); zero-padded `YYYY-MM-DD` Display; `pub(crate) const fn ymd`; string serde.
- provider.rs: rename to `Provider`; six variants spelled `typesafe, anthropic, openai, openrouter, nous, together`; delete `env_var()` (ids.rs:292-300, REQ-SAG-0401); doc says "OpenAI", never "Codex"; drop `#[non_exhaustive]`; error `TypesError::UnknownProvider`.
- short_name.rs: rename to `ShortName`, nine variants; error `TypesError::UnknownShortName`.
- session_id.rs: add imports (`std::{fmt, str::FromStr}`, serde, `ulid::Ulid`); error `TypesError::InvalidSessionId`; doc drops "since Phase A.1".
- tests: port to the new API; delete variant, family, ModelSource and override asserts and every `BackendId` line in names.rs:1-8; error asserts use `ErrorId::id()`; invalid-date and empty-id cases move to `ReleaseDate`/`ModelId` parse tests.

## 3. Target public API (exact; bodies elided)

```rust
// src/lib.rs
mod cancel; mod catalog; mod error; mod error_id; mod model; mod provider; mod release_date; mod session_id; mod short_name; mod var_name;
pub use cancel::{CancelHook, CancelToken, Wait};
pub use catalog::{Catalog, ModelChoice};
pub use error::TypesError;
pub use error_id::{ErrorDetail, ErrorId};
pub use model::{ModelDescriptor, ModelId, MODELS};
pub use var_name::VarName;
pub use provider::Provider;
pub use release_date::ReleaseDate;
pub use session_id::SessionId;
pub use short_name::ShortName;

// src/error_id.rs  (the one error convention of the workspace)
/// Every sc-agent error enum implements this. `id()` is stable per variant:
/// "<crate-prefix>.<variant_snake>", e.g. "http.provider_busy", "decision.schema_mismatch".
/// Open to workspace crates only (every sc-agent error enum implements it; it is not an extension
/// point for outside crates). `recovery()` and `detail()` are provided; `detail()` must not be
/// overridden (it is what keeps id, message, cause and recovery consistent across the workspace).
/// The cause method is `cause_detail`, not `cause`: `std::error::Error::cause` (deprecated) has the
/// same name on the supertrait and `x.cause()` would be ambiguous (E0034) wherever Error is in scope.
pub trait ErrorId: std::error::Error {
    fn id(&self) -> &'static str;
    /// This layer's own message, without the cause.
    fn message(&self) -> String;
    /// The lower error that caused this one, whole (its id, message, recovery and cause chain).
    fn cause_detail(&self) -> Option<ErrorDetail>;
    /// What the caller does next, if this layer knows. Provided: `None`. Sprints return non-empty
    /// text for secrets.key_missing (a-4), http.provider_busy and http.quota_exhausted (a-5),
    /// local.model_not_served (a-6), decide.no_provider_key and decide.request_parse (a-10).
    fn recovery(&self) -> Option<&'static str> { None }
    /// Provided: `ErrorDetail { id, message, recovery, cause: cause_detail() }`. Do not override.
    fn detail(&self) -> ErrorDetail;
}
/// An error as data: carries an error from a crate that is not below (an LlmError inside a
/// DecisionError) and is what the CLI, PyO3 and session rows print.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub id: String,
    pub message: String,
    /// Cow, not &'static str: ErrorDetail is also deserialized (session rows, PyO3), which cannot
    /// produce a &'static str. Values built from `ErrorId::recovery()` are Cow::Borrowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Cow<'static, str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ErrorDetail>>,
}
impl ErrorDetail {
    /// recovery = None.
    #[must_use] pub fn new(id: impl Into<String>, message: impl Into<String>, cause: Option<ErrorDetail>) -> Self;
    #[must_use] pub fn with_recovery(self, recovery: &'static str) -> Self;
    /// A foreign error (io, serde_json, sc-config, sc-observability, ureq, sc-sha): `id` is the
    /// foreign code or kind, `message` its Display; its `source()` chain becomes `cause`.
    #[must_use] pub fn foreign(id: impl Into<String>, error: &(dyn std::error::Error + 'static)) -> Self;
}
impl fmt::Display for ErrorDetail {}            // "<id>: <message>" only; the chain is in source()
impl std::error::Error for ErrorDetail {}       // source() = cause
// Rule for every crate: `impl Display for XError` writes "<id>: <message>" only (never the cause, so a
// reporter that walks source() prints each level once); `source()` returns the wrapped lower error;
// `detail()` carries the whole chain; wrapping keeps the lower value unchanged (REQ-SAG-0002).
// A lower error kept as a `kind: String` (io::ErrorKind etc.) is display-only text, not a stable
// format to match on.

// src/cancel.rs
#[derive(Clone, Default)]
pub struct CancelToken(Arc<Inner>);
struct Inner { cancelled: AtomicBool, state: Mutex<Hooks>, wake: Condvar }   // Debug hand-written
struct Hooks { next: u64, hooks: Vec<(u64, Arc<dyn Fn() + Send + Sync>)> }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait { Cancelled, DeadlineReached }
impl CancelToken {
    #[must_use] pub fn new() -> Self;
    pub fn cancel(&self);                                   // idempotent; wakes every waiter and runs every hook once
    #[must_use] pub fn is_cancelled(&self) -> bool;
    /// Condvar::wait_timeout_while; returns Cancelled at once if already cancelled. No sleep.
    #[must_use] pub fn wait_until(&self, deadline: Instant) -> Wait;
    /// Runs `wake` once on cancel (at once if already cancelled). `wake` must not block.
    #[must_use] pub fn on_cancel(&self, wake: Arc<dyn Fn() + Send + Sync>) -> CancelHook;
}
/// Deregisters its hook on drop.
#[must_use]
pub struct CancelHook { token: Weak<Inner>, id: u64 }
impl Drop for CancelHook {}
impl PartialEq for CancelToken { /* Arc::ptr_eq */ }  impl Eq for CancelToken {}
impl fmt::Debug for CancelToken {}

// src/model.rs
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelId(Cow<'static, str>);                      // non-empty, no control characters; provider-neutral
impl ModelId { #[must_use] pub fn as_str(&self) -> &str; pub(crate) const fn from_static(s: &'static str) -> Self; }
impl FromStr for ModelId { type Err = TypesError; }         // InvalidModelId
impl fmt::Display for ModelId {}  impl Serialize for ModelId {}  impl<'de> Deserialize<'de> for ModelId {}
impl AsRef<str> for ModelId {}  impl Borrow<str> for ModelId {}   // derived Eq/Ord/Hash on Cow<str> agree with str; no Deref (a ModelId is a validated value)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptor { model: ModelId, line: ShortName, released: ReleaseDate }
impl ModelDescriptor { pub const fn model(&self) -> &ModelId; pub const fn line(&self) -> ShortName; pub const fn released(&self) -> ReleaseDate; }
pub static MODELS: &[ModelDescriptor];                      // jev-1.13.0, jev-1.14.0, claude-haiku-5-5, claude-sonnet-5-5, claude-opus-5-5, claude-fable-5-2, gpt-6-luna, gpt-6.1-terra, gpt-6.1-sol, gpt-6-astra (dates above)

// src/short_name.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShortName { Jev, Haiku, Sonnet, Opus, Fable, Luna, Terra, Sol, Astra }
impl ShortName { pub const ALL: &'static [Self]; #[must_use] pub const fn as_str(self) -> &'static str; }
impl FromStr for ShortName { type Err = TypesError; }       // UnknownShortName
impl fmt::Display for ShortName {}  impl Serialize for ShortName {}  impl<'de> Deserialize<'de> for ShortName {}

// src/provider.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Provider { TypeSafe, Anthropic, OpenAi, OpenRouter, Nous, Together }
impl Provider {
    pub const ALL: &'static [Self];
    /// TypeSafe, Anthropic, OpenAi, OpenRouter, Together, Nous: the fixed priority ranking (REQ-SAG-0103).
    pub const PRIORITY: [Self; 6];
    /// The providers that serve `line`, in PRIORITY order; `None` = an id on no line.
    #[must_use] pub const fn ranking(line: Option<ShortName>) -> &'static [Self];
    #[must_use] pub const fn as_str(self) -> &'static str;
}
impl FromStr for Provider { type Err = TypesError; }        // UnknownProvider
impl fmt::Display for Provider {}  impl Serialize for Provider {}  impl<'de> Deserialize<'de> for Provider {}

// src/release_date.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An ordering key for "newest release wins", not a calendar date: it does not validate the calendar
/// (day 1..=31 is accepted for every month, e.g. 2026-02-31).
pub struct ReleaseDate { year: u16, month: u8, day: u8 }    // YYYY-MM-DD, month 1..=12, day 1..=31
impl FromStr for ReleaseDate { type Err = TypesError; }     // InvalidReleaseDate
impl fmt::Display for ReleaseDate {}  impl Serialize for ReleaseDate {}  impl<'de> Deserialize<'de> for ReleaseDate {}

// src/catalog.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog { latest: Latest, lines: BTreeMap<ModelId, ShortName> }
/// One slot per ShortName, so every line always has a model (total by construction).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Latest { jev: Slot, haiku: Slot, sonnet: Slot, opus: Slot, fable: Slot, luna: Slot, terra: Slot, sol: Slot, astra: Slot }
type Slot = (ModelId, ReleaseDate);
impl Latest {
    fn slot(&self, line: ShortName) -> &Slot;            // exhaustive match, no indexing
    fn slot_mut(&mut self, line: ShortName) -> &mut Slot;
}
impl Catalog {
    /// Seeds each slot from a per-line const row of MODELS, then offers every MODELS row (newer replaces).
    #[must_use] pub fn builtin() -> Self;
    /// Adds one global-config model; it becomes the line's latest when `released` is later or equal (equal: the config model wins).
    pub fn with_config_model(self, model: ModelId, line: ShortName, released: ReleaseDate) -> Result<Self, TypesError>; // ModelOnTwoLines
    /// Latest -> the line's slot; Version -> itself. Cannot fail.
    #[must_use] pub fn resolve(&self, choice: &ModelChoice) -> ModelId;
    /// doctor: is a provider-listed id in the catalog; takes &ModelId or a plain &str (no parse needed).
    #[must_use] pub fn line_of<Q>(&self, model: &Q) -> Option<ShortName> where ModelId: Borrow<Q>, Q: Ord + ?Sized;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelChoice { Latest(ShortName), Version(ModelId) }
impl FromStr for ModelChoice { type Err = TypesError; }     // short name first, else ModelId (InvalidModelId)
impl fmt::Display for ModelChoice {}

// src/var_name.rs
/// The name of an environment variable a config may point at (a local runner's key_env):
/// `^[A-Z_][A-Z0-9_]{0,127}$`. Holds no credential name (REQ-SAG-0401); sc-agent-secrets'
/// resolve_var and sc-agent-config's LocalRunner.key_env share it (A-4, REQ-SAG-0013).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarName(String);
impl VarName { #[must_use] pub fn as_str(&self) -> &str; }
impl FromStr for VarName { type Err = TypesError; }         // InvalidVarName
impl fmt::Display for VarName {}  impl AsRef<str> for VarName {}  impl Serialize for VarName {}  impl<'de> Deserialize<'de> for VarName {}

// src/session_id.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(Ulid);
impl SessionId { #[must_use] pub fn new() -> Self; }        // #[expect(clippy::new_without_default, reason = "each call is a new id")]
impl FromStr for SessionId { type Err = TypesError; }       // canonical 26-char uppercase only; InvalidSessionId
impl fmt::Display for SessionId {}  impl Serialize for SessionId {}  impl<'de> Deserialize<'de> for SessionId {}
```

## 4. Full error enum (src/error.rs, new)

Bottom crate: nothing is wrapped, `cause_detail()` is always `None`, `recovery()` is the default `None`. Display = `"<id>: <message>"`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypesError {
    /// Model id empty or with a control character.
    InvalidModelId { text: String },                                            // "types.invalid_model_id"
    /// Not one of the nine short names.
    UnknownShortName { text: String },                                          // "types.unknown_short_name"
    /// Not one of the six providers.
    UnknownProvider { text: String },                                           // "types.unknown_provider"
    /// Not YYYY-MM-DD with month 1..=12 and day 1..=31.
    InvalidReleaseDate { text: String },                                        // "types.invalid_release_date"
    /// A config model id already belongs to another line.
    ModelOnTwoLines { model: ModelId, line: ShortName, other_line: ShortName }, // "types.model_on_two_lines"
    /// Not an environment-variable name (`^[A-Z_][A-Z0-9_]{0,127}$`).
    InvalidVarName { text: String },                                            // "types.invalid_var_name"
    /// Not a canonical ULID (a UUID included).
    InvalidSessionId { text: String },                                          // "types.invalid_session_id"
}
impl ErrorId for TypesError {}  impl fmt::Display for TypesError {}  impl std::error::Error for TypesError {}
```

## 5. New code

- N1 `src/lib.rs` (starts with `#![forbid(unsafe_code)]`), `README.md`, crate `Cargo.toml` (`[lints] workspace = true`, no features).
- N2 `src/error.rs`, `src/error_id.rs` (section 3-4).
- N3 Workspace root `Cargo.toml`: `[workspace] members = ["crates/*"]`, `resolver = "3"`; `[workspace.package] edition = "2024"`, `rust-version = "1.94.1"`, `license = "MIT"`; `[workspace.lints.rust] unsafe_code = "deny"` (not `forbid`: PyO3's `#[pymodule]`/`#[pyfunction]` expansions carry `#[allow(unsafe_code)]`, which a workspace `forbid` rejects, so sc-treesitter-py (A-3) and sc-agent-py (A-10) would not compile); every non-PyO3 crate adds `#![forbid(unsafe_code)]` at its crate root (sc-agent-types does, in src/lib.rs); `[workspace.lints.clippy] panic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `indexing_slicing = "deny"` (REQ-SAG-0002 AC2; sc-lint does not enforce these yet, so clippy is the gate). `clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests` = true; `disallowed-methods = ["std::thread::sleep", "std::thread::sleep_until", { path = "sc_agent_secrets::ApiKey::as_str", reason = "the key is read only at sc-agent-http's ureq_transport (A-5)" }]` (REQ-SAG-0010 as a lint; the as_str entry is written here, see Shared files; A-5 does not edit clippy.toml).
- N4 `CancelToken::wait_until`, `on_cancel`, `CancelHook`, `Wait`.
- N5 `ModelChoice`, `Catalog::resolve` (infallible, total `Latest`), generic `line_of`, `VarName`.
- Tests (each names the acceptance line it proves; the acceptance list holds the assertions):
  - N6 tests/cancel.rs: proves AC "REQ-SAG-0104 (boundary part)" and "REQ-SAG-0104 AC2" (Send + Sync). Extra cases not in an acceptance line: `DeadlineReached` with no cancel; a hook that registers or drops another hook does not deadlock. Waits use deadlines and channels, never sleep.
  - N7 tests/catalog.rs: proves AC "REQ-SAG-0101 AC1", "REQ-SAG-0101 AC2", "REQ-SAG-0102 AC1/AC2" and "REQ-SAG-0102" (built-in dates). Extra cases: the same id on two lines is `types.model_on_two_lines`; every MODELS id round-trips through Display/serde; `line_of` for built-in and config ids, called with `&ModelId` and with `&str`; each `Latest` slot of `builtin()` is the newest MODELS row of its line.
  - N8 tests/names.rs: proves AC "REQ-SAG-0103" and "REQ-SAG-0001". Extra cases: ShortName/Provider/ReleaseDate/VarName spellings and rejections with their ids (VarName accepts MY_RUNNER_TOKEN (never a real credential name here, REQ-SAG-0401), rejects "", "lower", "1X", "A-B" and 129 characters).
  - N9 tests/error_id.rs: proves AC "REQ-SAG-0002" (both ErrorId lines). Extra cases: `ErrorDetail::foreign` keeps an io error's source chain; `cause_detail()` and `recovery()` called through `&dyn ErrorId` with `std::error::Error` in scope compile (the E0034 guard); an ErrorDetail with no recovery serializes without the key.

## 6. Dependencies

| Crate | Version (ref lock) | Why |
|---|---|---|
| serde | "1" (1.0.229), features ["derive"] | ErrorDetail; hand-written string serde for the name types |
| ulid | "1" (1.2.1) | SessionId |
| dev: serde_json | "1" (1.0.154) | round-trip tests |

No sc-observability: this crate does no I/O and logs nothing.

## 7. Not copied (reasons)

ModelId variant enum; ModelFamily; ModelSource, ResolvedModel and overrides (consumer concepts, REQ-SAG-0006); BackendDescriptor/BACKENDS, BackendId/BackendChoice/BackendSet (ADR-SAG-0001, REQ-SAG-0006); `env_var` (REQ-SAG-0401); digests and trace (sc-decision-schema, REQ-SAG-0202); RepoRoot, Rev, BlobSha, RepoRelativePath, git.rs (REQ-SAG-0301); ProjectLabel (A-8 Slug); AnswerId, Operation; JevError/code.rs/envelope.rs (replaced by ErrorId/ErrorDetail); secret.rs (A-4); observability/ (REQ-SAG-0008); schema/ (sc-treesitter); tests/model_freshness.rs (live calls).

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-1.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] Shared files: right after the copy commit a-1 pushes the workspace root Cargo.toml (members = ["crates/*"], [workspace.package], [workspace.lints]) and clippy.toml (sleep and sc_agent_secrets::ApiKey::as_str disallowed-methods entries); a-3, a-4 and a-8 start from that push; no later sprint edits either file.
- [ ] REQ-SAG-0101 AC1: sc-agent-types: each of jev, haiku, sonnet, opus, fable, luna, terra, sol, astra parses as ShortName and Catalog::resolve(Latest) (infallible: every line has a slot) returns the newest model of its line across built-in and with_config_model rows (test). The decide end-to-end check is a-10's.
- [ ] REQ-SAG-0101 AC2: sc-agent-types: a specific catalog version id parses as ModelChoice::Version and Catalog::resolve returns it unchanged (test).
- [ ] REQ-SAG-0102 AC1/AC2: sc-agent-types: Catalog::with_config_model adding a haiku model dated later than every built-in haiku makes resolve(Latest(Haiku)) return it with no rebuild; an equal date also makes it win; an earlier date changes nothing (tests). sc-agent-types reads no files (no std::fs use). Loading the row from JSON is a-4's.
- [ ] REQ-SAG-0102: sc-agent-types: every built-in MODELS row carries the model's published release date (claude-fable-5-2, not 5-1), checked by test, including jev-1.13.0 = 2026-09-15 (date supplied by the owner 2026-11-09).
- [ ] REQ-SAG-0103: sc-agent-types: Provider and ModelId are separate types; a catalog ModelId encodes no provider and is used with two Providers unchanged (test); Provider::PRIORITY is the fixed ranking and Provider::ranking(line) lists the providers serving a line in that order (test); Provider::OpenAi is spelled openai.
- [ ] REQ-SAG-0104 (boundary part; AC1 across crates is closed by a-9): sc-agent-types: a CancelToken cancelled on another thread ends wait_until with Wait::Cancelled at once (well before the deadline) and runs every registered on_cancel hook once (at once if registered after cancel; never after its CancelHook is dropped); clones share it (tests; no sleep).
- [ ] REQ-SAG-0104 AC2 (Python, boundary part): sc-agent-types: CancelToken is Send + Sync (compile test), so the PyO3 crate can wrap it; the Python test is a-10's (sc-agent-py).
- [ ] REQ-SAG-0001: sc-agent-types: SessionId::new() is a ULID; FromStr accepts only the canonical 26-char form and rejects a UUID with types.invalid_session_id (test).
- [ ] REQ-SAG-0002: sc-agent-types: ErrorId (id, message, cause_detail, provided recovery and detail) and ErrorDetail {id, message, recovery (optional, omitted when None), cause} exist; every TypesError variant has a distinct id types.<variant_snake>; Display is "<id>: <message>" only and detail() carries the chain; ErrorDetail round-trips through serde with its cause chain (tests).
- [ ] REQ-SAG-0002 AC2: workspace root Cargo.toml (a-1): denies clippy panic, unwrap_used, expect_used and indexing_slicing and sets rust unsafe_code = "deny" (not forbid, so PyO3 crates compile) for every member; sc-agent-types/src/lib.rs has #![forbid(unsafe_code)]; clippy.toml allows them only in tests and disallows std::thread::sleep/sleep_until; cargo clippy --workspace --all-targets -- -D warnings passes.
- [ ] REQ-SAG-0006: sc-agent-types: no type, field or constant names plan-anchor, judge-jev or a mock backend (review).
- [ ] REQ-SAG-0401: sc-agent-types: no credential env-var name appears in the crate (rg '_API_KEY' crates/sc-agent-types finds nothing).
- [ ] REQ-SAG-0009: sc-agent-types: `cargo tree -p sc-agent-types -e normal` lists no tokio crate (deps: serde, ulid; dev serde_json).
- [ ] REQ-SAG-0010: sc-agent-types source calls no sleep function (no std::thread::sleep, sleep_until or tokio::time::sleep): `rg 'sleep\(' crates/sc-agent-types/src` finds none, and the workspace clippy.toml that a-1 owns disallows std::thread::sleep and std::thread::sleep_until, so `cargo clippy --workspace -- -D warnings` fails on any sleep call.
- [ ] ADR-SAG-0004: no sc-agent crate may depend on a higher layer (types; config, secrets and log; http; decision schema; decision backends over sc-llm-provider; runner); sc-agent-types is the bottom layer: its Cargo.toml declares no sc-agent crate dependency (only serde and ulid; dev serde_json), checked by `cargo tree -p sc-agent-types`.
- [ ] REQ-SAG-0002: sc-agent-types: a test wraps a lower-layer error two levels deep (ErrorDetail::new with nested causes) and asserts the outermost ErrorDetail chain carries the id and message of every cause at every level.
- [ ] sc-agent-types: ModelId implements AsRef<str> and Borrow<str>, and Catalog::line_of answers for a plain &str without parsing (test); VarName parses ^[A-Z_][A-Z0-9_]{0,127}$ and rejects anything else with types.invalid_var_name (test).
- [ ] REQ-SAG-0013 (key_env pattern part): sc-agent-types: VarName accepts ^[A-Z_][A-Z0-9_]{0,127}$ and rejects anything else with types.invalid_var_name (tests/names.rs VarName test); a-4 applies it to key_env at load.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Same release date: the global-config model wins (assistant choice under the owner's 'IDC'); decide doctor warns (REQ-SAG-0503).
- Built-in rows and dates: research/model-dates.md; fable is claude-fable-5-2.
- Provider-specific ids: the catalog holds provider-neutral ids; each provider maps them (REQ-SAG-0103, sc-llm-provider A-7). Provider ranking is defined here (design).
- jev-1.13.0 release date 2026-09-15: date supplied by the owner 2026-11-09 (docs.typesafe.ai gives none; plan-hardening decision 10).
No open questions.
Hardening round 1 (2026-11-09): front matter added; a-1 owns workspace root Cargo.toml and clippy.toml (unsafe_code = "deny" at the workspace, #![forbid(unsafe_code)] per non-PyO3 crate) incl. the ApiKey::as_str entry (clippy 0.1.99 skips an entry whose crate is not a dependency, checked).
Hardening round 2 (2026-11-09): ErrorId::cause -> cause_detail (E0034 with Error::cause); recovery() provided + ErrorDetail.recovery; Display = "<id>: <message>"; LineHasNoModel deleted (total Latest, resolve infallible); ModelId AsRef/Borrow<str>, generic line_of; VarName added for A-4 key_env (REQ-SAG-0013); REQ-SAG-0104 AC1 -> a-9, REQ-SAG-0001 AC2 -> a-10.
QA-1 fix (2026-11-10, 5066236): TypesError::recovery() now returns Some for every variant (RBP-F001); short-name/provider lists moved from message() into recovery(); CancelHook drop waits for a running hook (REQ-QA-001). The design text saying recovery is the provided None is superseded.
