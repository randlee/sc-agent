<!-- Generated from bead agt-4p2.1 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-1: sc-agent-types
- id: `a-1` (bead `agt-4p2.1`)
- branch: `sprint/a-1-sc-agent-types`
- depends on: none
- copy script: `docs/plans/phase-a/copy/a-1.sh`

## Description

Build sc-agent-types. Governing records: REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0006, REQ-SAG-0101..0104. Code to copy from sc-agent-ref: see design.

## Design

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
- Built-in rows carry each model's published release date (REQ-SAG-0102; research/model-dates.md, checked 2026-11-09): jev-1.13.0 (date TODO: owner, not published), jev-1.14.0 2026-10-14, claude-haiku-5-5 2026-10-07, claude-sonnet-5-5 2026-09-28, claude-opus-5-5 2026-09-22, claude-fable-5-2 2026-11-09, gpt-6-luna 2026-09-22, gpt-6.1-terra 2026-10-26, gpt-6.1-sol 2026-09-29, gpt-6-astra 2026-09-03. Ids are provider-neutral (REQ-SAG-0103); each provider maps them to its own id in sc-llm-provider (A-7), e.g. OpenRouter `anthropic/claude-haiku-5.5`.
- Provider ranking (REQ-SAG-0103, assistant decision): `Provider::PRIORITY` = TypeSafe, Anthropic, OpenAi, OpenRouter, Together, Nous. `Provider::ranking(line)` is the providers that serve a line, in that order: jev -> [TypeSafe]; haiku, sonnet, opus, fable -> [Anthropic, OpenRouter, Together]; luna, terra, sol, astra -> [OpenAi, OpenRouter, Together]; an id on no line (`None`) -> [OpenRouter, Together, Nous]. decide takes the first with a detected key; `--provider` is only an optional override (A-10).
- `ModelChoice::Version(id)` resolves to itself without a catalog check (REQ-SAG-0101 AC2; REQ-SAG-0502/0503 allow unlisted models).
- `SessionId` (ULID) lives here: sc-agent-log, sc-decision-schema and the runner all use it.
- `CancelToken` has `wait_until(deadline)` and `on_cancel` registration; no sleep. The runner's queue registers a hook so held-off jobs and idle workers wake at once on cancel (A-9).
- Every crate's error enum implements `ErrorId` (below). Ids are `"<crate-prefix>.<variant_snake>"`. This replaces every SCREAMING_SNAKE code, `code()`/`lower_code()`/`lower_id()` method and serde-tag id scheme. No crate uses `thiserror`.

Size: about 420 lines of code and 200 lines of tests.

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
mod cancel; mod catalog; mod error; mod error_id; mod model; mod provider; mod release_date; mod session_id; mod short_name;
pub use cancel::{CancelHook, CancelToken, Wait};
pub use catalog::{Catalog, ModelChoice};
pub use error::TypesError;
pub use error_id::{ErrorDetail, ErrorId};
pub use model::{ModelDescriptor, ModelId, MODELS};
pub use provider::Provider;
pub use release_date::ReleaseDate;
pub use session_id::SessionId;
pub use short_name::ShortName;

// src/error_id.rs  (the one error convention of the workspace)
/// Every sc-agent error enum implements this. `id()` is stable per variant:
/// "<crate-prefix>.<variant_snake>", e.g. "http.provider_busy", "decision.schema_mismatch".
pub trait ErrorId: std::error::Error {
    fn id(&self) -> &'static str;
    /// This layer's own message, without the cause.
    fn message(&self) -> String;
    /// The lower error that caused this one, whole (its id, message and cause chain).
    fn cause(&self) -> Option<ErrorDetail>;
    /// Provided: `ErrorDetail { id, message, cause }`.
    fn detail(&self) -> ErrorDetail;
}
/// An error as data: carries an error from a crate that is not below (an LlmError inside a
/// DecisionError) and is what the CLI, PyO3 and session rows print.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ErrorDetail>>,
}
impl ErrorDetail {
    #[must_use] pub fn new(id: impl Into<String>, message: impl Into<String>, cause: Option<ErrorDetail>) -> Self;
    /// A foreign error (io, serde_json, sc-config, sc-observability, ureq, sc-sha): `id` is the
    /// foreign code or kind, `message` its Display; its `source()` chain becomes `cause`.
    #[must_use] pub fn foreign(id: impl Into<String>, error: &(dyn std::error::Error + 'static)) -> Self;
}
impl fmt::Display for ErrorDetail {}            // "<id>: <message>" then ": <cause>" for each cause
impl std::error::Error for ErrorDetail {}       // source() = cause
// Rule for every crate: `impl Display for XError` writes `self.detail()`; `source()` returns the
// wrapped lower error; wrapping keeps the lower value unchanged (REQ-SAG-0002).

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
pub struct ReleaseDate { year: u16, month: u8, day: u8 }    // YYYY-MM-DD, month 1..=12, day 1..=31
impl FromStr for ReleaseDate { type Err = TypesError; }     // InvalidReleaseDate
impl fmt::Display for ReleaseDate {}  impl Serialize for ReleaseDate {}  impl<'de> Deserialize<'de> for ReleaseDate {}

// src/catalog.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog { latest: BTreeMap<ShortName, (ModelId, ReleaseDate)>, lines: BTreeMap<ModelId, ShortName> }
impl Catalog {
    #[must_use] pub fn builtin() -> Self;
    /// Adds one global-config model; it becomes the line's latest when `released` is later or equal (equal: the config model wins).
    pub fn with_config_model(self, model: ModelId, line: ShortName, released: ReleaseDate) -> Result<Self, TypesError>; // ModelOnTwoLines
    pub fn resolve(&self, choice: &ModelChoice) -> Result<ModelId, TypesError>; // Latest -> newest of line (LineHasNoModel); Version -> itself
    #[must_use] pub fn line_of(&self, model: &ModelId) -> Option<ShortName>;   // doctor: is a provider-listed id in the catalog
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelChoice { Latest(ShortName), Version(ModelId) }
impl FromStr for ModelChoice { type Err = TypesError; }     // short name first, else ModelId (InvalidModelId)
impl fmt::Display for ModelChoice {}

// src/session_id.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(Ulid);
impl SessionId { #[must_use] pub fn new() -> Self; }        // #[expect(clippy::new_without_default, reason = "each call is a new id")]
impl FromStr for SessionId { type Err = TypesError; }       // canonical 26-char uppercase only; InvalidSessionId
impl fmt::Display for SessionId {}  impl Serialize for SessionId {}  impl<'de> Deserialize<'de> for SessionId {}
```

## 4. Full error enum (src/error.rs, new)

Bottom crate: nothing is wrapped, `cause()` is always `None`. Display = `detail()`.

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
    /// A line has no model (cannot happen with the built-in rows; returned instead of a panic).
    LineHasNoModel { line: ShortName },                                         // "types.line_has_no_model"
    /// Not a canonical ULID (a UUID included).
    InvalidSessionId { text: String },                                          // "types.invalid_session_id"
}
impl ErrorId for TypesError {}  impl fmt::Display for TypesError {}  impl std::error::Error for TypesError {}
```

## 5. New code

- N1 `src/lib.rs`, `README.md`, crate `Cargo.toml` (`[lints] workspace = true`, no features).
- N2 `src/error.rs`, `src/error_id.rs` (section 3-4).
- N3 Workspace root `Cargo.toml`: `[workspace] members = ["crates/*"]`, `resolver = "3"`; `[workspace.package] edition = "2024"`, `rust-version = "1.94.1"`, `license = "MIT"`; `[workspace.lints.rust] unsafe_code = "forbid"`; `[workspace.lints.clippy] panic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `indexing_slicing = "deny"` (REQ-SAG-0002 AC2; sc-lint does not enforce these yet, so clippy is the gate). `clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests` = true; `disallowed-methods = ["std::thread::sleep", "std::thread::sleep_until"]` (REQ-SAG-0010 as a lint; A-5 adds its `ApiKey::as_str` entry).
- N4 `CancelToken::wait_until`, `on_cancel`, `CancelHook`, `Wait`.
- N5 `ModelChoice`, `Catalog::resolve`, `line_of`.
- N6 tests/cancel.rs: a clone sees cancel; a token cancelled from another thread ends `wait_until` with `Cancelled` well before a far deadline; `DeadlineReached` with no cancel; already-cancelled returns at once; a hook runs once on cancel, at once when registered after cancel, and never after its `CancelHook` is dropped; a hook that registers or drops another hook does not deadlock; `CancelToken: Send + Sync`. Waits use deadlines and channels, never sleep.
- N7 tests/catalog.rs: each short name resolves to its newest row; a later config `haiku` row wins; an equal-date one wins; an earlier one changes nothing; every built-in row has the release date listed above; the same id on two lines is `types.model_on_two_lines`; `Version(id)` resolves to itself (catalog and unlisted ids); every MODELS id round-trips through Display/serde; `line_of` for built-in and config ids.
- N8 tests/names.rs: ShortName/Provider/ReleaseDate/SessionId spellings and rejections with their ids; the same `ModelId` used with two `Provider`s unchanged (REQ-SAG-0103); `Provider::ranking` returns the lists above and `PRIORITY` the fixed order.
- N9 tests/error_id.rs: every TypesError id is distinct and matches `^types\.[a-z_]+$`; Display equals `detail().to_string()`; `ErrorDetail` with a two-level cause serializes to `{"id","message","cause":{...}}` and back; `foreign` keeps an io error's source chain.

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
- [ ] REQ-SAG-0101 AC1: each of jev, haiku, sonnet, opus, fable, luna, terra, sol, astra parses as ShortName and Catalog::resolve(Latest) returns the newest model of its line across built-in and config rows (test).
- [ ] REQ-SAG-0101 AC2: a specific catalog version id parses as ModelChoice::Version and resolves to itself (test).
- [ ] REQ-SAG-0102 AC1/AC2: with_config_model adding a haiku model dated later than every built-in haiku makes haiku resolve to it with no rebuild; an equal date also makes it win; an earlier date changes nothing (tests). sc-agent-types reads no files (no std::fs use).
- [ ] REQ-SAG-0102: every built-in MODELS row carries the model's published release date (claude-fable-5-2, not 5-1); jev-1.13.0's date is supplied by the owner before the sprint closes.
- [ ] REQ-SAG-0103: Provider and ModelId are separate types; a catalog ModelId encodes no provider and is used with two Providers unchanged (test); Provider::PRIORITY is the fixed ranking and Provider::ranking(line) lists the providers serving a line in that order (test); Provider::OpenAi is spelled openai.
- [ ] REQ-SAG-0104 AC1: a CancelToken cancelled on another thread ends wait_until with Wait::Cancelled at once (well before the deadline) and runs every registered on_cancel hook once (at once if registered after cancel; never after its CancelHook is dropped); clones share it; CancelToken is Send + Sync (tests; no sleep).
- [ ] REQ-SAG-0104 AC2 (Python): CancelToken is Send + Sync so the PyO3 crate can wrap it; the Python test lives in the PyO3 crate.
- [ ] REQ-SAG-0001: SessionId::new() is a ULID; FromStr accepts only the canonical 26-char form and rejects a UUID with types.invalid_session_id (test).
- [ ] REQ-SAG-0002: ErrorId (id, message, cause, detail) and ErrorDetail {id, message, cause} exist in sc-agent-types; every TypesError variant has a distinct id types.<variant_snake>; Display equals detail(); ErrorDetail round-trips through serde with its cause chain (tests).
- [ ] REQ-SAG-0002 AC2: the workspace root Cargo.toml denies clippy panic, unwrap_used, expect_used and indexing_slicing for every member; clippy.toml allows them only in tests and disallows std::thread::sleep/sleep_until; cargo clippy --workspace -- -D warnings passes.
- [ ] REQ-SAG-0006: no type, field or constant names plan-anchor, judge-jev or a mock backend (review).
- [ ] REQ-SAG-0401: no credential env-var name appears in sc-agent-types (grep).
- [ ] REQ-SAG-0009/0010: cargo tree -p sc-agent-types lists no tokio crate (deps: serde, ulid; dev serde_json); no sleep call in non-test source.
- [ ] ADR-SAG-0004: no sc-agent crate may depend on a higher layer (types; config, secrets and log; http; decision schema; decision backends over sc-llm-provider; runner); sc-agent-types is the bottom layer: its Cargo.toml declares no sc-agent crate dependency (only serde and ulid; dev serde_json), checked by `cargo tree -p sc-agent-types`.
- [ ] REQ-SAG-0002: a test wraps a lower-layer error two levels deep and asserts the outermost error's ErrorDetail chain carries the id and message of every cause at every level.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Same release date: the global-config model wins (assistant choice under the owner's 'IDC'); decide doctor warns (REQ-SAG-0503).
- Built-in rows and dates: research/model-dates.md; fable is claude-fable-5-2.
- Provider-specific ids: the catalog holds provider-neutral ids; each provider maps them (REQ-SAG-0103, sc-llm-provider A-7). Provider ranking is defined here (design).
Remaining owner item: the release date of jev-1.13.0 (docs.typesafe.ai gives none); marked TODO in the design.
