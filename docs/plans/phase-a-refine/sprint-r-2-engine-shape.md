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
  - REQ-SAG-0600
  - REQ-SAG-0601
  - REQ-SAG-0602
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0710
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0013
  - ADR-SAG-0014
  - ADR-SAG-0015
  - ADR-SAG-0019
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0024
  - ADR-SAG-0027
owned_paths:
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
  - "docs/sc-jev-py/architecture.md"
  - "docs/architecture.md#ADR-SAG-0003 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0005 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0009 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0019 amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0022 amendment (r-2)"
  - "docs/architecture.md#Rust API patterns amendment (r-2)"
  - "docs/architecture.md#ADR-SAG-0024"
  - "docs/requirements.md#REQ-SAG-0312, 0313, 0314, 0607, 0620 rows; REQ-SAG-0601 amendment"
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
       pub fn mode(self, mode: MultiBackendMode) -> Self; // honoured from r-2b
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
5. **`RunResponse`** (`#[non_exhaustive]`, private fields, accessors):
   `session_id: Option<SessionId>` (`None` when `session_saved` is false,
   CPR-M6), `session_saved`, `prompt_sha256`, `payload_bytes: u64`,
   `dry_run`, `equal_input`, `results: Vec<BackendResult>`, dry-run
   `payload` and `limits: Vec<BackendLimit { backend, request_limit_bytes, over_limit }>`.
   `BackendResult { backend, status }`;
   `#[non_exhaustive] enum BackendStatus { Ok(JudgeOutcome), Failed(BackendFailure) }`
   where `BackendFailure` holds the `ScAgentErrorResponse` (serialized) and the
   in-memory `JevError` (not serialized). `RunResponse::first_failure() ->
   Option<&BackendFailure>` and `is_partial() -> bool` give every consumer one
   exit/exception decision. In r-2 `run` is still fail-fast, so no status is
   `Failed` yet (r-2b).
6. **One serialization.** `RunResponse` has one hand-written `Serialize`,
   which **is** the ADR-SAG-0014 `data` body. The primary backend is the first
   `Ok` result in `BackendSet` order: if it is Jev, its `provider_meta` fields
   then `answers` are copied to the top level verbatim; if it is an LLM
   backend, `answers`, `model`, `usage`. `primary_backend` names it. Then
   `session_id`, `session_saved`, `prompt_sha256`, `payload_bytes`,
   `results`, `equal_input` (dry-run: `payload`, `limits`). The
   `{version, operation, data}` wrapper moves beside it as
   `sc_jev_check::run::Envelope { version, operation, data }` with
   `Envelope::run(&RunResponse)`, `Envelope::ask(..)` and
   `with_readback(impl Serialize)` (the only consumer-added key). It replaces
   `merge_jev_response`/`run_data` in `sc-ai` and the shaping in
   `sc-jev-py/src/run.rs`; there is no `to_envelope_data`. Pinned samples
   (target `run_response`, fixtures under `crates/sc-jev-check/tests/fixtures/run-response/`):
   - `single_jev.json` — `primary_backend: "jev"`, Jev fields at top level,
     `results: [{backend: "jev", status: "ok", outcome: {...}}]`.
   - `jev_haiku.json` — Jev primary; two `ok` results.
   - `jev_failed_haiku_ok.json` — `primary_backend: "haiku"`, Haiku
     `answers` at top level, `results[0] = {backend: "jev", status: "failed", error: {error: "NETWORK_ERROR", message: ..., retryable: true}}`,
     `equal_input.leak: true` (produced from r-2b on; r-2 pins it by
     constructing the `RunResponse` directly).
   - `dry_run.json` — `payload`, `limits`, `equal: false`, all `NotSent`.
7. **The four old entry points are deleted with no shims:** `build_request`,
   `run_check`, `run_request`, `prepare_caller_request`, and `CheckResult`,
   `BuiltRequest`, `RunRequest`, `PreparedCallerRequest`, `RunOptions`.
   Python `run_check` keeps its Python name and calls `prepare` + `run`.
8. **Registry** (EXT-015, EXT-016): `BackendRegistry::from_env(repo_root: &RepoRoot, backends: &BackendSet) -> Result<Self, JevError>`
   (keys via `sc_agent_secrets::resolve_key`) and
   `BackendRegistry::from_resolver(backends, impl Fn(BackendKeyId) -> Result<ResolvedApiKey, JevError>)`
   are public (production registry for `ask` consumers without copying
   `sc-ai`); `BackendRegistry::register(Arc<dyn JudgeBackend>)` rejects a
   duplicate id; the built-in factory arm (r-1) is the only id dispatch.
   Injecting a registry through `RunConfig::registry` is the documented
   no-secrets path for embedders (EXT-009).
9. `boundaries/sc-jev-check/jev-check.toml` facade lists exactly the items
   above plus `config::{load_check_at, CheckSpec, InputSpec}`,
   `session::FileSessionStore`, `registry::{BackendRegistry, register_scripted_backends}`.

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

Land: ADR-SAG-0024 → Active (header "Amends ADR-SAG-0005, ADR-SAG-0009,
ADR-SAG-0019, ADR-SAG-0022"); ADR-SAG-0005 r-2 amendment (entry point
`prepare`, `run::build_request` sentence superseded); ADR-SAG-0009 r-2
amendment (callers use `prepare`/`run` with `RunConfig::backends`; the
`run_check(..., backends)` sentence is superseded); ADR-SAG-0022 r-2
amendment (`RunConfig::registry`; "`run_check`'s parameters stay unchanged"
superseded); ADR-SAG-0019 r-2 amendment (`RunConfig`); ADR-SAG-0003 r-2
amendment (toplevel repo root); Rust API patterns Extractor note withdrawn.
REQ-SAG-0601 amended: "`prepare`/`run` library API; dry-run via
`RunConfig::dry_run`". `docs/sc-jev-py/architecture.md` line 25: the
`build_request` sentence now names `prepare` (ADR-SAG-0024).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0620 | Facade lists only `prepare`/`run`/`discover_repo_root` as run entry points (sc-lint facade check); `test_run_response_contract` pins the four samples; `test_backend_precedence`; `test_prepared_run_owns_config` |
| req:REQ-SAG-0607 | `SessionStore` trait and `install_registry` gone (compile); `test_session_written_once_only_with_llm`; `test_ask_takes_registry`; concurrency tests above |
| req:REQ-SAG-0312 | `test_command_path_placeholder_from_check_toml`; `test_command_placeholder_without_path_is_config_invalid`; `test_command_path_symlink_outside_root_refused` |
| req:REQ-SAG-0313 | `test_command_stderr_error_response_reraised`; `test_command_stderr_unknown_code_is_extraction_failed` |
| req:REQ-SAG-0314 | No `refuse_over_cap` on the request path; no `max_input_bytes`; `test_payload_between_old_caps_refused_by_backend_before_send`; `test_dry_run_reports_over_limit` |
| adr:ADR-SAG-0024 | Status Active; text matches signatures; header lists the amended ADRs |
| adr:ADR-SAG-0005, 0009, 0019, 0022, 0003 | r-2 subsections Active |
| adr:ADR-SAG-0021 | Path precedence tests in `sc-ai` and `sc-jev-py` unchanged and green |
| boundary:jev-check | Regression: README parity rows, cross-impl digests, session 0600/expiry tests; `cargo tree -p sc-jev-check --depth 1` has no `ureq` |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev-check --test run_response -- test_run_response_contract --exact
cargo test -p sc-jev-check --test run_response -- test_backend_precedence --exact
cargo test -p sc-jev-check --test run_response -- test_prepared_run_owns_config --exact
cargo test -p sc-jev-check --test run_response -- test_repo_root_from_subdirectory --exact
cargo test -p sc-jev-check --test run_response -- test_dry_run_reports_over_limit --exact
cargo test -p sc-jev-check --test session -- test_session_written_once_only_with_llm --exact
cargo test -p sc-jev-check --test session -- test_ask_takes_registry --exact
cargo test -p sc-jev-check --test session -- test_two_stores_same_dir_last_writer_wins --exact
cargo test -p sc-jev-check --test session -- test_save_error_removes_tmp --exact
cargo test -p sc-jev-check --test session -- test_stale_tmp_swept --exact
cargo test -p sc-jev-check --test command -- test_command_path_placeholder_from_check_toml --exact
cargo test -p sc-jev-check --test command -- test_command_placeholder_without_path_is_config_invalid --exact
cargo test -p sc-jev-check --test command -- test_command_path_symlink_outside_root_refused --exact
cargo test -p sc-jev-check --test command -- test_command_stderr_error_response_reraised --exact
cargo test -p sc-jev-check --test command -- test_command_stderr_unknown_code_is_extraction_failed --exact
cargo test -p sc-jev-check --test caps -- test_payload_between_old_caps_refused_by_backend_before_send --exact
cargo test -p sc-ai --test cli
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
test -z "$(cargo tree -p sc-jev-check -e normal --depth 1 | grep ureq)"
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Per-backend failure semantics, independent/chain modes, digest-based `equal`
(r-2b); consumer partial-failure output, `--chain-context`, Python kwarg
(r-3b); `sc-agent-compose`, the e2e producer test and the cross-consumer
parity test (r-3b); the HTTP crate (r-3); D9 (r-5).
