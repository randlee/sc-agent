---
id: r-2
phase: r
status: planned
closure_type: boundary
target_boundary: jev-check
track: runtime
wave: 2
size: L
branch: sprint/r-2-engine-shape
worktree: sprint/r-2-engine-shape
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [r-1]
parallel_safe: [r-3, r-3c]
vertical_rationale: "The four deleted entry points have callers in sc-ai and sc-jev-py; D2 forbids shims, so consumer call sites move in the same commit. No consumer behaviour changes beyond the pinned response shape."
requirements:
  - REQ-SAG-0620
  - REQ-SAG-0607
  - REQ-SAG-0312
  - REQ-SAG-0313
  - REQ-SAG-0314
  - REQ-SAG-0305
  - REQ-SAG-0300
  - REQ-SAG-0301
  - REQ-SAG-0302
  - REQ-SAG-0502
  - REQ-SAG-0503
  - REQ-SAG-0600
  - REQ-SAG-0601
  - REQ-SAG-0602
  - REQ-SAG-0603
  - REQ-SAG-0610
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0710
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
  - REQ-SAG-0800
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0007
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0010
  - ADR-SAG-0011
  - ADR-SAG-0013
  - ADR-SAG-0014
  - ADR-SAG-0015
  - ADR-SAG-0017
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0023  # binding once r-4 has made its r-4 part Active; r-2 does not cite its r-3b subsection
  - ADR-SAG-0024
  - ADR-SAG-0027
  - ADR-SAG-0029
  # ADR-SAG-0008/0014/0019 bind through their r-1 interim amendments (Active, r-1),
  # landed by the r-1 QA-1 fix round; replace this comment with the amendment id if one is assigned.
owned_paths:
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-jev-check/src/**"
  - "crates/sc-jev-check/tests/**"
  - "crates/sc-jev-check/assets/jev.toml"
  - "crates/sc-ai/src/command.rs"
  - "crates/sc-ai/src/envelope.rs"
  - "crates/sc-ai/src/lib.rs"
  - "crates/sc-ai/tests/cli.rs"
  - "crates/sc-jev-py/src/run.rs"
  - "crates/sc-jev-py/src/lib.rs"
  - "tests/fixtures/jev-python/**"
  - "boundaries/sc-jev-check/**"
  - "docs/sc-jev-check/**"
  - "docs/sc-jev-py/architecture.md#Contract (the `build_request` sentence)"
  - "docs/architecture.md#ADR-SAG-0003 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0005 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0014 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0014 body inline marker 'Amended by ADR-SAG-0024 (Proposed, r-2)'"
  - "docs/architecture.md#ADR-SAG-0009 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0019 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0022 amendment (r-2)"
  - "docs/architecture.md#Rust API patterns amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0024"
  - "docs/architecture.md#ADR index row ADR-SAG-0024"
  - "docs/architecture.md#ADR index Amended-by marker (r-2) in ADR-SAG-0003, 0005, 0009, 0014, 0019, 0022, 0029"
  - "docs/requirements.md#REQ-SAG-0312, 0313, 0314, 0607, 0620 rows; REQ-SAG-0601, REQ-SAG-0710 amendments"
---

# Sprint r-2 — engine shape (D2 engine, D4, D11 engine, D13 engine)

## Outcome

`sc-jev-check` has exactly one public way to run a check: `prepare` then
`run`, returning one `RunResponse` with one serialization that both consumers
print unchanged. The session store is a stateless file store with a written
concurrency model. The command producer contract is usable from `check.toml`
and reports producer errors with their own codes. The decorative `Extractor`
trait is gone, and the engine no longer applies its own byte cap.

## Closes findings

ARCHE-003, ARCHE-004, ARCHE-007, ARCHE-011, ARCHE-012, EXT-004, EXT-006,
EXT-009, RBP-F005, RBP-F008, RBP-F009, RBQA-F004; contributes EXT-002,
ARCHE-006/CR-001/ARCHP-002 (contract; e2e in r-3b), RBQA-F006 (caller),
ARCH-006 (phase-end, `prepare` sentence).

Labels `(Active, r-N)` in the quoted ADR wording below are the post-flip form of the
`(Proposed, r-N)` markers used elsewhere in this doc.

## Deliverables

### D2 — single engine API (REQ-SAG-0620, ADR-SAG-0024)

1. `crates/sc-jev-check/src/run/mod.rs`:
   ```rust
   #[non_exhaustive]
   pub enum CheckSource { CheckFile(PathBuf), Spec(CheckSpec), CallerRequest(CallerRequest) }

   pub struct CallerRequest { /* private; parsed, validated */ }
   impl CallerRequest { pub fn from_json(bytes: &[u8]) -> Result<Self, JevError>; }

   pub struct RunConfig { /* private fields */ }
   impl RunConfig {
       pub fn new(repo_root: RepoRoot, session_log: SharedSessionLog) -> Self;
       pub fn backends(self, backends: BackendSet) -> Self;
       pub fn rev(self, rev: Rev) -> Self;
       pub fn dry_run(self, on: bool) -> Self;
       pub fn system_prompt_override(self, path: impl Into<PathBuf>) -> Self;
       pub fn sessions_dir(self, dir: impl Into<PathBuf>) -> Self;
       pub fn registry(self, registry: Arc<BackendRegistry>) -> Self;
       pub fn cancel(self, token: CancelToken) -> Self;
   }

   pub struct PreparedRun { /* private: owns RunConfig, Arc<PayloadBundle>, resolved prompt */ }
   impl PreparedRun {
       pub fn prompt_sha256(&self) -> &Sha256Digest;
       pub fn payload_bytes(&self) -> u64;
       pub fn producer(&self) -> Option<&SubjectsProducer>;
       pub fn request(&self) -> &JudgeRequest;
       pub fn backends(&self) -> &BackendSet;
   }

   pub fn prepare(source: CheckSource, config: RunConfig) -> Result<PreparedRun, JevError>;
   pub fn run(prepared: PreparedRun) -> Result<RunResponse, JevError>;
   pub fn discover_repo_root(start: &Path) -> Result<RepoRoot, JevError>;
   ```
   `prepare` takes the config by value and `run` consumes the `PreparedRun`, so
   a run cannot see a different config than the one it was prepared with
   (RBP-F101, EXT-017). `run` builds the production registry from the config
   (`BackendRegistry::from_env`) unless `registry` was set.
   `RunConfig` has **no `mode` setter in r-2**: no r-2 deliverable reads a
   mode, and the setter's only governing text is ADR-SAG-0025 (Proposed,
   r-2b), so r-2b adds `RunConfig::mode(MultiBackendMode)` together with
   ADR-SAG-0025 (plan "ADR ownership"). r-2's `run` records the r-1
   default mode on `EqualInputRecord`, as r-1 does.
2. **Backend precedence** (CPR-011): `RunConfig::backends` when set (CLI
   `--backend`, Python `backends=`), else the source document's `backends`
   (check.toml or caller request), else `[jev]`. `test_backend_precedence`
   (target `run_response`) covers all three.
3. **Static facts only before construction.** `prepare` and dry-run read
   `BackendDescriptor` (`requires_llm_context` decides whether the system
   prompt is resolved; `request_limit_bytes` feeds dry-run) and construct no
   backend.
4. **`discover_repo_root`** calls `sc_agent_types::git::discover_toplevel`
   from `start`; outside git it returns `start`, absolutized. Both consumers
   call it; this ends the git-toplevel vs cwd disagreement. Behaviour change
   recorded in the ADR-SAG-0003 r-2 amendment: `sc-jev-py` called from a
   subdirectory now reads the toplevel `.env` (`test_repo_root_from_subdirectory`,
   target `run_response`; pytest `test_dotenv_read_from_toplevel_in_subdir`).
   `extract/read.rs` `read_git_show` is replaced by
   `sc_agent_types::git::read_at_rev` (RBQA-F006 caller).
5. **`RunResponse`** (private fields and accessors; no `#[non_exhaustive]`,
   which private fields make redundant): `session_id: Option<SessionId>`
   (`None` when `session_saved` is false, CPR-M6), `session_saved`,
   `prompt_sha256`, `payload_bytes: u64`, `dry_run`, `equal_input`,
   `results: Vec<BackendResult>`, `warnings: Vec<String>`, dry-run `payload`
   and `limits: Vec<BackendLimit { backend, request_limit_bytes, over_limit }>`.
   `BackendResult { backend, status }`;
   `#[non_exhaustive] enum BackendStatus { Ok(JudgeOutcome), Failed(BackendFailure) }`.
   `BackendFailure` stores **only** the `JevError`; its serialized `error`
   object is derived at serialize time as
   `ScAgentErrorResponse::from_jev_error(&error)`, so the two can never
   diverge (`error()` and `response()` accessors). `RunResponse::first_failure() ->
   Option<&BackendFailure>` and `is_partial() -> bool` give every consumer one
   exit/exception decision. `RunResponse::with_warning(self, impl Into<String>) -> Self`
   appends a warning; it is the one way a consumer adds data to the body
   (r-3b routes the Phase A log-flush warning, CR-014, through it; r-2b adds
   the session-save warning). `RunResponse::test_builder()` (behind the
   `sc-jev-check` `test-util` feature, plan "Test seams") builds any shape,
   including `Failed` statuses, for the pinned samples. In r-2 `run` is still
   fail-fast, so no status is `Failed` yet at runtime (r-2b).
6. **One serialization.** `RunResponse` has one hand-written `Serialize`,
   which **is** the ADR-SAG-0014 `data` body (ADR-SAG-0014 r-2 amendment,
   REQ-SAG-0710 amendment). The primary backend is the first `Ok` result in
   `BackendSet` order. If it is Jev, every `provider_meta` field **except
   `answers`** is copied to the top level verbatim, then `answers` is written
   once, from the typed `answers` (r-1 D10.1: `choice`, `confidence`,
   `probabilities`, byte-identical to the Jev answer objects for those
   keys). There are no duplicate keys: the `provider_meta` `answers` key is
   skipped, never written a second time, and `results[].outcome` keeps the
   full `provider_meta`. If the primary is an LLM backend, `answers`,
   `model`, `usage`. `primary_backend` names it. Then
   `session_id`, `session_saved`, `prompt_sha256`, `payload_bytes`,
   `results`, `equal_input` (dry-run: `payload`, `limits`), and last
   `warnings` (omitted when empty, so the first four samples are unaffected).
   The `{version, operation, data}` wrapper moves beside it as
   `sc_jev_check::run::Envelope { version, operation, data }` with
   `Envelope::run(&RunResponse) -> Envelope`,
   `Envelope::ask(outcome: &JudgeOutcome, session_id: &SessionId) -> Envelope` and
   `with_readback(impl Serialize)` (the only consumer-added envelope key;
   warnings go through `RunResponse::with_warning`). It replaces
   `merge_jev_response`/`run_data` in `sc-ai` and the shaping in
   `sc-jev-py/src/run.rs`; there is no `to_envelope_data`. Pinned samples
   (target `run_response`, built with `RunResponse::test_builder`, fixtures
   under `crates/sc-jev-check/tests/fixtures/run-response/`):
   - `single_jev.json` — `primary_backend: "jev"`, Jev fields at top level,
     built from a real a-10 Jev response body (answers carry `confidence`);
     the top-level `answers` equals that body's `answers` byte for byte and
     the key appears once;
     `results: [{backend: "jev", status: "ok", outcome: {...}}]`.
   - `jev_haiku.json` — Jev primary; two `ok` results.
   - `jev_failed_haiku_ok.json` — `primary_backend: "haiku"`, Haiku
     `answers` at top level (no Jev answer anywhere at top level),
     `results[0] = {backend: "jev", status: "failed", error: {error: "NETWORK_ERROR", message: ..., schema_version: 1, retryable: true}}`,
     `equal_input.leak: true` (produced at runtime from r-2b on; r-2 pins it
     through `test_builder`).
   - `dry_run.json` — `payload`, `limits`, `equal: false`, all `NotSent`.
   - `session_save_failed.json` — Haiku `ok`, `session_saved: false`,
     `session_id: null`, `warnings: ["session not saved: …"]` (produced at
     runtime from r-2b on; r-2 pins it through `test_builder`).

   **`equal_input` in the samples is a shape pin.** Every sample carries
   `equal_input` serialized exactly as the Active r-1 `EqualInputRecord`
   (r-1 interim amendment of ADR-SAG-0008/0014/0019); r-2 adds no field and
   changes no spelling. `leak = any Failed`, how each `Attestation` value is
   derived and the session-save-failure runtime behaviour are ADR-SAG-0025
   (r-2b), so `jev_failed_haiku_ok.json` and `session_save_failed.json` are
   built with `RunResponse::test_builder` and claim no r-2 runtime
   behaviour. `dry_run.json` is produced by a real dry run: nothing is sent,
   so every backend is `NotSent` and `equal` is false (ADR-SAG-0024 r-2
   bullet below).

   The integration targets that need `test-util` (at least `run_response`,
   for `test_builder`) are declared
   `[[test]] name = "<t>" required-features = ["test-util"]`, and every
   validation line for an `sc-jev-check` integration target passes
   `--features test-util`. There is **no self dev-dependency**: r-1's D0
   spike (`boundaries/README.md` "Dev-dependency and self edges") found
   that sc-boundary flags a path-only self edge (`SCB-MANIFEST-002`) and a
   versioned one cannot be packaged. r-2 owns `crates/sc-jev-check/Cargo.toml`
   except r-1's `test-util` feature line,
   `test-util = ["sc-agent-types/test-util"]`, which it keeps; it switches
   `sc-agent-types` to `features = ["git"]` for D2.4 and adds the `[[test]]`
   entries. ADR-SAG-0022's sentence that this crate's own tests see the
   scripted registry through `cfg(test)` is corrected in the ADR-SAG-0022
   r-2 amendment, whose "Correction" sentence r-2 rewords (see ADR text).
7. **The four old entry points are deleted with no shims:** `build_request`,
   `run_check`, `run_request`, `prepare_caller_request`, and `CheckResult`,
   `BuiltRequest`, `RunRequest`, `PreparedCallerRequest`, `RunOptions`.
   Python `run_check` keeps its Python name and calls `prepare` + `run`.
8. **Registry** (EXT-015, EXT-016): `BackendRegistry::from_env(repo_root: &RepoRoot, backends: &BackendSet) -> Result<Self, JevError>`
   (keys via `sc_agent_secrets::resolve_key`) and
   `BackendRegistry::from_resolver(backends, impl Fn(BackendKeyId) -> Result<ResolvedApiKey, JevError>)`
   are public (production registry for `ask` consumers without copying
   `sc-ai`); `BackendRegistry::register(Arc<dyn JudgeBackend>)` installs a
   double or scripted backend for an **existing** id and rejects a duplicate;
   the r-1 `FACTORIES` table is the only id dispatch, and engine branching
   reads descriptor facts (r-1 D1.6).
   Injecting a registry through `RunConfig::registry` is the documented
   no-secrets path for embedders (EXT-009).
9. `boundaries/sc-jev-check/jev-check.toml` facade lists exactly the items
   above plus `config::{load_check_at, CheckSpec, InputSpec}`,
   `session::FileSessionStore`, `registry::{BackendRegistry, register_scripted_backends}`
   (`register_scripted_backends` and `RunResponse::test_builder` under
   `test-util`). `registry::ScriptedJudge` and `registry::RequestLog` are not
   in it: r-1 deleted them (r-1 D10.11). `FACTORIES` stays private (its
   coverage test is a `--lib` test).

### D4 — stateless session store (REQ-SAG-0607)

1. `FileSessionStore { dir, ttl, clock: fn() -> SystemTime }` (fn pointer; no
   `Arc<dyn Fn>`), `new(dir)`, `with_ttl`, `with_clock`,
   `save(&self, &StoredSession)`, `load(&self, &SessionId)`,
   `ask(&self, id, follow_up: &str, registry: &BackendRegistry, log: &SharedSessionLog) -> Result<JudgeOutcome, JevError>`.
2. Delete `trait SessionStore`, `install_registry`, `record_outcome`, `open`,
   `load_raw`, the `RwLock` and the `Mutex`.
3. **Concurrency model** (RSH-004), stated in rustdoc and ADR-SAG-0024: no
   lock. `save` writes `<id>.json.tmp.<pid>.<random>` with `create_new`,
   fsyncs it, renames over `<id>.json`, fsyncs the parent, and removes the tmp
   file on any error. Last writer wins; readers are protected by the a-10
   digest and id re-verification. `ask` does **not** rewrite the session file.
   `new` sweeps `*.tmp.*` files older than 1 h. Tests (target `session`):
   `test_two_stores_same_dir_last_writer_wins`, `test_save_error_removes_tmp`,
   `test_stale_tmp_swept`.
4. `run` writes the session **once**, at the end, only when a backend whose
   descriptor has `supports_session_ask` returned `Ok`
   (`test_session_written_once_only_with_llm`, target `session`). a-10's 0600
   file / 0700 dir rules are kept.

### D11 — command producer contract (REQ-SAG-0312, REQ-SAG-0313)

1. `config::InputSpec::Command` gains `path: Option<RepoRelativePath>`,
   validated by the same rules as `file` inputs (regular file, canonicalized,
   `starts_with` the canonical repo root — the a-10 rule), and substituted for
   `{path}` with a-10's single-pass substitution. `{path}` in argv with no
   `path` is `CONFIG_INVALID`.
2. The child keeps a-10's environment contract: `env_clear()` then the
   allowlist (`PATH`, `HOME`, `USERPROFILE`, `SYSTEMROOT`, `TMPDIR`, `TEMP`,
   `TMP`, `LANG`, `LC_ALL`); no API-key variable is ever passed (RSH-001).
3. On non-zero exit, `extract/command.rs` reads the last non-empty stderr line
   (bounded by the stderr cap); if it parses as `ScAgentErrorResponse` it
   returns `JevError::from_error_response` (`ProducerError` with the producer
   name from the input), otherwise `ExtractionFailed`.
4. Fake producer scripts live in `crates/sc-jev-check/tests/fixtures/producers/`;
   the real-binary e2e test lands in r-3b.

### D13 — engine cap removed (REQ-SAG-0314)

1. Delete `pub trait Extractor`, `mod private { Sealed }`, `extractor_for`;
   `extract_input` matches the four `InputSpec` kinds.
2. The backend owns its byte cap (r-1: descriptor limit read by
   `validate_for_send`). Delete `payload::refuse_over_cap` from the request
   path and `max_input_bytes` (200 000) from `PackagedConfig` and
   `assets/jev.toml`. `run` calls `validate_for_send` on every selected
   backend before the first `judge` (ADR-SAG-0004). Dry-run constructs no
   backend and reports `limits[i].over_limit = payload_bytes > request_limit_bytes`
   for each selected backend; it does not fail. a-10 per-file read caps stay
   as resource guards.

### ADR text

Land: ADR-SAG-0024 → Active (header "Amends ADR-SAG-0003, ADR-SAG-0005,
ADR-SAG-0009, ADR-SAG-0014, ADR-SAG-0019, ADR-SAG-0022"); ADR-SAG-0014 r-2 amendment
(`data` is the `RunResponse` serialization; primary backend = first `Ok` in
`BackendSet` order, which may be an LLM backend; `warnings` replaces the
Phase A `data.warning`; "verbatim Jev response" holds only when Jev is
primary); ADR-SAG-0005 r-2 amendment (entry point
`prepare`, `run::build_request` sentence superseded); ADR-SAG-0009 r-2
amendment (callers use `prepare`/`run` with `RunConfig::backends`; the
`run_check(..., backends)` sentence is superseded); ADR-SAG-0022 r-2
amendment (`RunConfig::registry`; "`run_check`'s parameters stay unchanged"
superseded; the `cfg(test)` correction supersedes, for that one sentence,
ADR-SAG-0029 (b)'s "Everything else in ADR-SAG-0022 stands"; 0029 (b)'s
`_testing.run_check_scripted` seam and string-shorthand rule are unchanged);
ADR-SAG-0019 r-2 amendment (`RunConfig`); ADR-SAG-0003 r-2
amendment (toplevel repo root); Rust API patterns Extractor note withdrawn.

**ADR-SAG-0024 bullet added before the flip** (r-2 owns ADR-SAG-0024 and
inserts this bullet after the `RunResponse` bullet, then flips the whole
ADR; plan review 4, pre-screen r-2 fix 2):

> - (Active, r-2) `equal_input` is the serialized r-1 `EqualInputRecord`
>   (r-1 interim amendment of ADR-SAG-0008/0014/0019): r-2 pins its
>   serialized shape in every `RunResponse` sample and adds no field. A
>   dry run sends nothing, so every backend is `NotSent` and `equal` is
>   false. `leak = any Failed`, the derivation of every other `Attestation`
>   value and the session-save-failure behaviour (`session_saved = false`,
>   one `warnings` entry, answers kept) are ADR-SAG-0025 (r-2b); until
>   r-2b the samples that show them (`jev_failed_haiku_ok.json`,
>   `session_save_failed.json`) are built with `RunResponse::test_builder`
>   and are shape pins only. `RunConfig` has no `mode` setter until r-2b
>   (ADR-SAG-0025).

**ADR-SAG-0022 r-2 amendment, "Correction" sentence reworded** (D0
outcome 4 of r-1, `boundaries/README.md`). Replace
"`sc-jev-check`'s integration targets enable `test-util` through a
self-referencing dev-dependency (plan.md "Test seams")" with:

> `sc-jev-check`'s integration targets enable `test-util` through
> `[[test]] required-features = ["test-util"]` and `--features test-util`
> on their validation lines (no self dev-dependency; `boundaries/README.md`
> "Dev-dependency and self edges").

**Markers r-2 flips and markers it must not touch.** r-2 flips: the
ADR-SAG-0024 Status line and index row; its own r-2 subsections of 0003,
0005, 0009, 0014, 0019, 0022 and the Rust API patterns amendment; the
"(Proposed, r-2)" markers in index rows 0003, 0005, 0009, 0014, 0019, 0022
and 0029; and the inline "Amended by ADR-SAG-0024 (Proposed, r-2)" line
under the ADR-SAG-0014 body. It must **not** touch the r-2b markers that
share cells with them: "ADR-SAG-0025 (Proposed, r-2b)" in index rows 0019
and 0022 (and rows 0004 and 0008), the r-2b subsections of 0019 and 0022,
or the r-1, r-3, r-4 and r-5 markers in rows 0005, 0009, 0022 and 0029.
REQ-SAG-0601 amended: "`prepare`/`run` library API; dry-run via
`RunConfig::dry_run`". REQ-SAG-0710 amended: "`data` is the `RunResponse`
serialization (ADR-SAG-0014 r-2 amendment); the primary backend is the first
`Ok` backend in `BackendSet` order; its answers are at the top level".
The ADR index row ADR-SAG-0024 and the r-2 "Amended by" markers in rows
0003, 0005, 0009, 0014, 0019, 0022, 0029 flip to Active.
`docs/sc-jev-py/architecture.md`, section "Contract": the sentence that says
`build_request` calls `sc_jev_check::run::build_request` now names `prepare`
(ADR-SAG-0024). The sentence is anchored by section, not line, because a-10
rewrites the lines around it.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0620 | Facade lists only `prepare`/`run`/`discover_repo_root` as run entry points (sc-lint facade check); `test_run_response_contract` pins the five samples (incl. the Haiku-primary `jev_failed_haiku_ok.json` and `session_save_failed.json`); `test_backend_failure_response_derived_from_error`; `test_backend_precedence`; `test_prepared_run_owns_config` |
| req:REQ-SAG-0710 (amended) | `jev_failed_haiku_ok.json`: Haiku primary, `primary_backend: "haiku"`, no Jev fields at top level; `single_jev.json`: Jev response verbatim at top level, answers with `confidence`, `answers` key once (`test_run_response_contract`) |
| req:REQ-SAG-0601 (amended) | `cargo test -p sc-jev-check --features test-util --test run_response -- test_run_response_contract --exact` and `-- test_dry_run_reports_over_limit --exact` each print `1 passed`: `prepare`/`run` is the library API and `RunConfig::dry_run` produces `dry_run.json` |
| req:REQ-SAG-0607 | `SessionStore` trait and `install_registry` gone (compile); `test_session_written_once_only_with_llm`; `test_ask_takes_registry`; concurrency tests above |
| req:REQ-SAG-0312 | `test_command_path_placeholder_from_check_toml`; `test_command_placeholder_without_path_is_config_invalid`; `test_command_path_symlink_outside_root_refused` |
| req:REQ-SAG-0313 | `test_command_stderr_error_response_reraised`; `test_command_stderr_unknown_code_is_extraction_failed` |
| req:REQ-SAG-0314 | No `refuse_over_cap` on the request path; no `max_input_bytes`; `test_payload_between_old_caps_refused_by_backend_before_send`; `test_dry_run_reports_over_limit` |
| adr:ADR-SAG-0024 | Status Active; text matches signatures (no `RunConfig::mode`); the `equal_input` shape-pin bullet above is present verbatim; header lists the amended ADRs incl. 0003 and 0014; index row Active |
| adr:ADR-SAG-0005, 0009, 0014, 0019, 0022, 0003, 0029 | r-2 subsections Active; Rust API patterns amendment (r-2) Active; the r-2 "Amended by" markers in index rows 0003, 0005, 0009, 0014, 0019, 0022 and 0029 and the inline marker under the ADR-SAG-0014 body read Active; the ADR-SAG-0022 r-2 "Correction" sentence reads as reworded above; the r-2b markers in rows 0004, 0008, 0019 and 0022 still read "(Proposed, r-2b)" |
| adr:ADR-SAG-0002, 0007, 0017, 0020, 0023 | Regression: cross-impl digests unchanged (0002); `cargo test -p sc-jev-backends --test llm` green and no backend file changed (0007); `cargo publish --dry-run` below exits 0 for the published set (0017); `BackendRegistry::from_env` reads keys only through `sc_agent_secrets::resolve_key` and the command child keeps a-10's `env_clear()` allowlist with no API-key variable (0020, D11.2; `cargo test -p sc-jev-check --features test-util --test command` green); `bash .github/scripts/check-exact-pins.sh` green (0023, r-4 part) |
| adr:ADR-SAG-0021 | Path precedence tests in `sc-ai` and `sc-jev-py` unchanged and green |
| boundary:jev-check | Regression: README parity rows, cross-impl digests, session 0600/expiry tests; `cargo tree -p sc-jev-check --depth 1` has no `ureq`; no `#[allow(` under `crates/`. Per added id: REQ-SAG-0300/0301/0302 `cargo test -p sc-jev-check --lib -- extract::` and `test_repo_root_from_subdirectory`; REQ-SAG-0502 `test_payload_between_old_caps_refused_by_backend_before_send`; REQ-SAG-0503 `cargo test -p sc-jev-check --lib -- run::prompt::`; REQ-SAG-0600 `cargo test -p sc-jev-check --features test-util --test sessions_exact`; REQ-SAG-0603 `test_run_response_contract` (`jev_haiku.json`); REQ-SAG-0610 `cargo test -p sc-jev-check --lib -- run::tests::`; REQ-SAG-0305 `cargo test -p sc-jev-check --features test-util --test command`; REQ-SAG-0602 `test_run_response_contract` (`equal_input` in every sample); REQ-SAG-0700/0701 `cargo test -p sc-ai --test cli`; NFR-SAG-0001 `cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact`; NFR-SAG-0002 `cargo test -p sc-jev-backends --test llm` and `cargo test -p sc-ai --test cli`; NFR-SAG-0003 `cargo test -p sc-jev-check --features test-util --test sessions_exact`; REQ-SAG-0800 pytest below |
| boundary:test-util | `cargo tree -p sc-jev-check -e normal,features` shows no `test-util` feature (dev-only) |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev-check --features test-util --test run_response -- test_run_response_contract --exact
cargo test -p sc-jev-check --features test-util --test run_response -- test_backend_failure_response_derived_from_error --exact
cargo test -p sc-jev-check --features test-util --test run_response -- test_backend_precedence --exact
cargo test -p sc-jev-check --features test-util --test run_response -- test_prepared_run_owns_config --exact
cargo test -p sc-jev-check --features test-util --test run_response -- test_repo_root_from_subdirectory --exact
cargo test -p sc-jev-check --features test-util --test run_response -- test_dry_run_reports_over_limit --exact
cargo test -p sc-jev-check --features test-util --test session -- test_session_written_once_only_with_llm --exact
cargo test -p sc-jev-check --features test-util --test session -- test_ask_takes_registry --exact
cargo test -p sc-jev-check --features test-util --test session -- test_two_stores_same_dir_last_writer_wins --exact
cargo test -p sc-jev-check --features test-util --test session -- test_save_error_removes_tmp --exact
cargo test -p sc-jev-check --features test-util --test session -- test_stale_tmp_swept --exact
cargo test -p sc-jev-check --features test-util --test command -- test_command_path_placeholder_from_check_toml --exact
cargo test -p sc-jev-check --features test-util --test command -- test_command_placeholder_without_path_is_config_invalid --exact
cargo test -p sc-jev-check --features test-util --test command -- test_command_path_symlink_outside_root_refused --exact
cargo test -p sc-jev-check --features test-util --test command -- test_command_stderr_error_response_reraised --exact
cargo test -p sc-jev-check --features test-util --test command -- test_command_stderr_unknown_code_is_extraction_failed --exact
cargo test -p sc-jev-check --features test-util --test caps -- test_payload_between_old_caps_refused_by_backend_before_send --exact
cargo test -p sc-jev-check --features test-util --test sessions_exact
cargo test -p sc-jev-check --features test-util --test command
cargo test -p sc-jev-check --lib -- extract::
cargo test -p sc-jev-check --lib -- run::
cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact
cargo test -p sc-jev-backends --test llm
cargo test -p sc-ai --test cli
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
test -z "$(cargo tree -p sc-jev-check -e normal --depth 1 | grep -E '(^|[^[:alnum:]_-])ureq v[0-9]')"
test -z "$(cargo tree -p sc-jev-check -e normal,features | grep -F '"test-util"')"
! rg -n '#!?\[allow\(' crates/
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Per-backend failure semantics, independent/chain modes, digest-based `equal`
(r-2b); consumer partial-failure output, `--chain-context`, Python kwarg
(r-3b); `sc-agent-compose`, the e2e producer test and the cross-consumer
parity test (r-3b); the HTTP crate (r-3); D9 (r-5).
