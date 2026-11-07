---
id: r-1
phase: r
status: planned
closure_type: contract
target_boundary: agent-types
track: foundation
wave: 1
branch: sprint/r-1-types-break
worktree: sprint/r-1-types-break
recommended_agent: rust-architect
recommended_model: opus
must_follow: [a-10]
parallel_safe: [r-4]
requirements:
  - REQ-SAG-0105
  - REQ-SAG-0106
  - REQ-SAG-0107
  - REQ-SAG-0108
  - REQ-SAG-0101
  - REQ-SAG-0103
  - REQ-SAG-0104
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0009
  - ADR-SAG-0016
  - ADR-SAG-0027
owned_paths:
  - "crates/sc-agent-types/**"
  - "docs/sc-agent-types/**"
  - "crates/sc-jev/src/**"
  - "crates/sc-jev-backends/src/**"
  - "crates/sc-jev-check/src/**"
  - "crates/sc-jev-py/src/**"
  - "crates/sc-ai/src/command.rs"
  - "crates/sc-ai/src/envelope.rs"
  - "crates/sc-ai/src/lib.rs"
  - "crates/sc-ai/src/observe.rs"
  - "crates/sc-treesitter/src/render.rs"
  - "crates/sc-treesitter/src/extract.rs"
  - "crates/sc-treesitter/src/lib.rs"
---

# Sprint r-1 — sc-agent-types break (D1, D6, D10-break, D7 types part)

## Outcome

One deliberate pre-1.0 break of `sc-agent-types` before the first crates.io
publish. After it, the backend set and the error enum are open
(`#[non_exhaustive]`), the engine reads backend facts from capabilities and a
single catalog instead of matching ids, errors are explicit variants with no
string-prefix parsing, and `sc-agent-types` no longer links
`sc-observability-types`. Call sites in every crate are updated in the same
sprint so the workspace stays green. Both `main.rs` files are excluded; r-4
owns them.

**Overrides the a-1 module freeze** (sprint-a-1 deliverable 8 and its module
layout): decisions D7 and D6 add a `schema` module (replacing `subjects`) and
replace the third-party `EventError` re-export with a local `LogRejected` in
`observability::v2`. No other module is added. The catalog lives in `ids.rs` and
capabilities in `judge.rs`.

## Closes findings

EXT-001, ARCHE-005, ARCHC-005, RBP-F004 (backend catalog part), RBP-F001,
ARCHC-004, ARCHE-022, EXT-007, ARCHP-003, RBP-F002 (taxonomy part), ARCHC-011,
EXT-011, RBQA-F010 (probabilities part), RBQA-F009, ARCHC-008, ARCHC-021,
ARCHP-001 (validator/builder part), ARCHP-020; a-10 wave-1 follow-ups
"serde spelling of `openai_luna`" and "`encode_failed`/`io_failed` code".

## Deliverables

### D1 — open backend set (REQ-SAG-0105, ADR-SAG-0009 amendment)

1. `#[non_exhaustive]` on `BackendId`, `BackendKeyId`, `JevError`, and every
   published struct in the crate that a-10 did not already mark.
2. `crates/sc-agent-types/src/ids.rs`: `pub struct BackendDescriptor { pub id: BackendId,
   pub name: &'static str, pub aliases: &'static [&'static str], pub key_id: Option<BackendKeyId>,
   pub family: BackendFamily }` and `pub static BACKENDS: &[BackendDescriptor]`
   (rows: `jev`, `haiku`, `openai_luna`). `BackendId::{as_str, key_id, family}`,
   `Display`, and the a-10 `FromStr` all read `BACKENDS`. No other `match` on
   `BackendId` remains outside `ids.rs` (enforced by a test that greps for
   `BackendId::` match arms in `crates/*/src`, allowlisting `ids.rs` and the
   backend crates' own `id()` impls).
3. `#[non_exhaustive] pub enum BackendFamily { TypeSafeJev, Llm, Local }`.
4. `crates/sc-agent-types/src/judge.rs`: `#[non_exhaustive] pub struct BackendCapabilities
   { pub family: BackendFamily, pub requires_llm_context: bool, pub supports_session_ask: bool,
   pub request_limit_bytes: u64 }` and a **required** trait method
   `fn capabilities(&self) -> BackendCapabilities` on `JudgeBackend`. Implemented by
   `JevClient`, Haiku, Luna and the scripted test backend. The engine replaces
   `registry::backend_key_id`, the LLM-context check in `run`, session-ask
   eligibility, and `equal_input::commands_for` id matches with
   `capabilities()` / `BACKENDS` lookups.
5. **Serde spelling fix** (a-10 follow-up). `BackendId::OpenAiLuna` becomes
   `#[serde(rename = "openai_luna", alias = "open_ai_luna")]` and
   `BackendKeyId::OpenAi` becomes `#[serde(rename = "openai", alias = "open_ai")]`.
   Serialization emits only the documented spelling. The alias exists so that
   session files written by Phase A still load, and it is removed in the
   following minor release (bead `agt-backend-serde-alias-removal`). This
   changes `JudgeOutcome.backend` on the wire. Update the a-10 test that pinned
   the old serde form to pin the new one, and add an alias-read test.

### D6 — error taxonomy (REQ-SAG-0106, ADR-SAG-0027)

1. Replace every prefix-encoded `ConfigInvalid { detail }` constructor with an
   explicit variant. The `code()` id of each is unchanged and pinned by
   `test_code_ids_unchanged` against `tests/fixtures/jev/README-contract/errors.json`:
   - `CredentialStoreInvalid { what, cause, recovery }` (from `credential_store_invalid`)
   - `RegistryInvalid { recovery }` (from `registry_invalid`)
   - `SessionStoreIo { op: &'static str, kind: std::io::ErrorKind, recovery }` (from `session_store_io`)
   - `Operational { area: OperationalArea, recovery }` with `#[non_exhaustive] enum OperationalArea { Config, Io }` (from `operational_config` / `operational_io`)
   - `SessionDocumentInvalid { recovery }`
2. Producer-neutral variants: `LanguageUnavailable { producer: String, language: String }`,
   `QueryInvalid { producer, detail }`, `QueryNotFound { producer, query }`.
   `sc-treesitter` passes `producer = "sc-treesitter"`.
3. `NetworkError` gains `kind: NetworkErrorKind` (`#[non_exhaustive]`: `Busy`,
   `Timeout`, `Connect`, `Tls`, `Status`, `BodyTooLarge`).
   `pub fn retryable(&self) -> bool` returns true only for
   `NetworkError { kind: Busy | Timeout | Connect, .. }`.
4. `encode_failed` / `io_failed` (added by a-10) become variants
   `EncodeFailed { what, cause }` and `IoFailed { what, kind }`. Both keep code
   `INPUT_INVALID`. No new stable code is added in A.1, because `errors.json`
   is the Python parity contract. ADR-SAG-0027 records this choice and the
   trigger for revisiting it.
5. `fix_hint()` becomes an exhaustive `match` on variants. Delete the four
   `*_DETAIL_PREFIX` constants and the `strip_prefix` chain (`error.rs:9-29`, `:425-428`).
6. `std::error::Error::source()` stays `None` on purpose: causes are redacted
   `String`s (secret hygiene, ADR-SAG-0020). This is documented on the type.
7. `pub fn from_error_response(resp: &ScAgentErrorResponse) -> JevError` maps a
   known code to its variant, carrying the message as `recovery`/`detail`. An
   unknown code becomes `ExtractionFailed` with the code in the detail. r-2
   consumes this.

### D10-break — outcome shape

1. Remove `JudgeOutcome.probabilities`; `answers` already holds them. `sc-jev`
   keeps validating probabilities strictly (tighten #3) before building `answers`.
2. Add `JudgeOutcome.input_sha256: Sha256Digest`: the digest, computed by the
   backend, of the canonical bytes it actually embedded on the wire. Each
   backend computes it in its wire builder. r-2b consumes it. This lands here so
   the frozen contract breaks only once.
3. Add `#[non_exhaustive] pub enum MultiBackendMode { #[default] Independent, ChainContext }`
   and `EqualInputRecord.{mode: MultiBackendMode, backends: Vec<BackendAttestation>}`
   with `pub struct BackendAttestation { pub backend: BackendId, pub input_sha256: Sha256Digest, pub ok: bool }`.
   In r-1 the engine fills them with the current behaviour; r-2b changes the
   semantics.
4. Rustdoc on `JudgeBackend` gains a `# Blocking` section: calls block the
   calling thread, and async hosts must use `spawn_blocking`. The trait stays
   synchronous (D10).

### D7 — types scope (REQ-SAG-0107, REQ-SAG-0108)

1. `observability::v2`: `pub struct LogRejected { reason: String }` (with `Display`
   and `Error`) replaces the re-exported `EventError`. `SessionLog::emit` returns
   `Result<(), LogRejected>`. Drop `sc-observability-types` from
   `crates/sc-agent-types/Cargo.toml`. The root workspace entry is removed by r-3.
   The `sc-ai` and `sc-jev-py` adapters map `EventError → LogRejected`.
2. `schema` module: `schema::subjects` (moved from `subjects.rs`: `SubjectsDocument`,
   `Subject`, `Span`, `SubjectsProducer`, `SubjectsSource`, `SUBJECTS_SCHEMA_V1`) and
   `schema::check` (`InputSpec`). Crate-root re-exports of moved items are
   removed, so each type has one path.
3. `SubjectsDocument::validate(&self) -> Result<(), JevError>` moves in from
   `sc-jev-check/src/extract/subjects.rs::validate_subjects_document`.
   `pub struct SubjectsBuilder` (`new(source, producer)`, `push_subject(path, span, text, kind)`,
   `build() -> Result<SubjectsDocument, JevError>`) assigns deterministic ids
   using the algorithm currently in `sc-treesitter/src/render.rs`. Both
   `sc-treesitter` (producer) and `sc-jev-check` (consumer) call these. The copies
   in `render.rs` and `extract/subjects.rs` are deleted.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0105 | `BACKENDS` is the only id table; `capabilities()` required; no `BackendId` match outside `ids.rs`; `test_backend_catalog_roundtrip`, `test_no_backend_id_matches_outside_catalog` |
| req:REQ-SAG-0106 | No `*_DETAIL_PREFIX` in tree; every variant's `code()` unchanged (`test_code_ids_unchanged`); `test_retryable_only_network_transient`; `test_from_error_response_known_and_unknown` |
| req:REQ-SAG-0107 | `cargo tree -p sc-agent-types -e normal` lists no `sc-observability*`; `test_log_rejected_display` |
| req:REQ-SAG-0108 | `test_subjects_builder_ids_match_golden` (golden fixture unchanged); `sc-treesitter` and `sc-jev-check` call `SubjectsBuilder`/`validate` (grep test) |
| adr:ADR-SAG-0027 | Amendment text matches code; `#[non_exhaustive]` on `JevError` |
| adr:ADR-SAG-0009 | Amendment bullet: adding a backend = variant + `BACKENDS` row + `capabilities()` |
| adr:ADR-SAG-0016 | `LogRejected` bullet landed |
| boundary:agent-types | Cross-impl digests unchanged (`jev_cross_impl`, `cross_impl_branches`); `test_backend_serde_documented_spelling`, `test_backend_serde_reads_old_alias` |

## Required validation

```bash
just lint
just test
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test -p sc-agent-types -- --nocapture
cargo test -p sc-agent-types -- test_code_ids_unchanged --exact
cargo test -p sc-agent-types -- test_backend_catalog_roundtrip --exact
cargo test -p sc-agent-types -- test_backend_serde_documented_spelling --exact
cargo test -p sc-agent-types -- test_backend_serde_reads_old_alias --exact
cargo test -p sc-agent-types -- test_retryable_only_network_transient --exact
cargo test -p sc-agent-types -- test_subjects_builder_ids_match_golden --exact
cargo test -p sc-jev-check cross_impl_branches -- --nocapture
test -z "$(cargo tree -p sc-agent-types -e normal | grep sc-observability)"
cargo publish --dry-run --locked -p sc-agent-types
```

## This sprint does not close

Engine API and session store (r-2), multi-backend semantics (r-2b), HTTP crate
and `ResolvedApiKey::new` sealing (r-3), composition crate (r-3b), pins and
allocator (r-4), D9 (r-5), typed `JudgeMeta` / async trait (Phase B), new
stable error codes.
