---
id: r-2
phase: r
status: planned
closure_type: boundary
target_boundary: jev-check
track: runtime
wave: 2
branch: sprint/r-2-engine-shape
worktree: sprint/r-2-engine-shape
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [r-1]
parallel_safe: [r-3]
requirements:
  - REQ-SAG-0620
  - REQ-SAG-0607
  - REQ-SAG-0312
  - REQ-SAG-0313
  - REQ-SAG-0314
  - REQ-SAG-0600
  - REQ-SAG-0601
  - REQ-SAG-0305
adrs:
  - ADR-SAG-0004
  - ADR-SAG-0019
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0024
owned_paths:
  - "crates/sc-jev-check/src/**"
  - "crates/sc-jev-check/tests/**"
  - "crates/sc-jev-check/assets/jev.toml"
  - "crates/sc-jev-backends/src/validate.rs"
  - "crates/sc-jev/src/check.rs"
  - "crates/sc-agent-types/src/schema/check.rs"
  - "crates/sc-ai/src/command.rs"
  - "crates/sc-ai/src/envelope.rs"
  - "crates/sc-ai/tests/cli.rs"
  - "crates/sc-jev-py/src/run.rs"
  - "crates/sc-jev-py/src/lib.rs"
  - "docs/sc-jev-check/**"
---

# Sprint r-2 — engine shape (D2 engine, D4, D11, D13 code)

## Outcome

`sc-jev-check` has exactly one public way to run a check: `prepare` then
`run`, returning one `RunResponse` that both consumers serialize unchanged.
The session store is a stateless file store with no hidden registry. The
command producer contract is usable from `check.toml` and reports producer
errors with their own codes. The decorative `Extractor` trait is gone. Each
backend is the single owner of its request byte cap.

## Closes findings

EXT-002 (engine part), ARCHE-003, ARCHE-011, ARCHE-012, RBQA-F005 (engine part),
QA-004, RBP-F013 (engine part), EXT-006, ARCHE-004, RBP-F005, RBP-F008, CR-001,
ARCHE-006, ARCHP-002 (contract part; e2e proof in r-3b), ARCHE-007, RBP-F009,
EXT-004, RBQA-F004 (byte cap).

## Deliverables

### D2 — single engine API (REQ-SAG-0620, ADR-SAG-0024)

1. `crates/sc-jev-check/src/run/mod.rs`:
   ```rust
   #[non_exhaustive]
   pub enum CheckSource { CheckFile(PathBuf), Spec(CheckSpec), CallerRequest(serde_json::Value) }

   #[non_exhaustive]
   pub struct RunConfig { /* builder: */ }
   impl RunConfig {
       pub fn new(repo_root: RepoRoot, backends: Vec<BackendId>, session_log: SharedSessionLog) -> Self;
       pub fn rev(self, rev: Rev) -> Self;
       pub fn dry_run(self, on: bool) -> Self;
       pub fn system_prompt_override(self, path: impl Into<PathBuf>) -> Self;
       pub fn sessions_dir(self, dir: impl Into<PathBuf>) -> Self;
       pub fn registry(self, registry: BackendRegistry) -> Self;
       pub fn mode(self, mode: MultiBackendMode) -> Self; // honoured from r-2b
   }

   pub struct PreparedRun { /* private fields; accessors: */ }
   impl PreparedRun {
       pub fn prompt_sha256(&self) -> &Sha256Digest;
       pub fn payload_bytes(&self) -> usize;
       pub fn producer(&self) -> Option<&SubjectsProducer>;
       pub fn request(&self) -> &JudgeRequest;
   }

   pub fn prepare(source: CheckSource, config: &RunConfig) -> Result<PreparedRun, JevError>;
   pub fn run(prepared: PreparedRun, config: &RunConfig) -> Result<RunResponse, JevError>;
   pub fn discover_repo_root(start: &Path) -> Result<RepoRoot, JevError>;
   ```
   `discover_repo_root` returns `git rev-parse --show-toplevel` from `start`
   (argv-only, `GIT_TERMINAL_PROMPT=0`, a-10 rules). Outside git it returns
   `start`, absolutized. Both consumers call it; this ends the git-toplevel vs
   cwd disagreement.
2. `RunResponse` (`#[non_exhaustive]`, `Serialize`):
   `{ session_id: SessionId, session_saved: bool, prompt_sha256, payload_bytes: usize, dry_run: bool,
   equal_input: EqualInputRecord, results: Vec<BackendResult> }`, where
   `BackendResult { backend: BackendId, status: BackendStatus }` and
   `#[non_exhaustive] enum BackendStatus { Ok(JudgeOutcome), Failed { code: String, message: String } }`.
   In r-2, `run` stays fail-fast, so every status is `Ok`; r-2b adds `Failed`.
   `pub fn to_envelope_data(&self) -> serde_json::Value` produces the
   ADR-SAG-0014 `data` body (Jev answer JSON plus `session_id`). It replaces
   `merge_jev_response` in `sc-ai` and the equivalent shaping in `sc-jev-py/src/run.rs`.
3. **The four old entry points are deleted with no shims:** `build_request`,
   `run_check`, `run_request`, `prepare_caller_request`, together with
   `CheckResult`, `BuiltRequest`, `RunRequest`, `PreparedCallerRequest`,
   `RunOptions`. Reason: nothing has been published to crates.io, every
   consumer is in this workspace and is migrated in this sprint, and shims
   would make a fifth surface. The Python function `run_check` keeps its
   Python name and calls `prepare` + `run`.
4. The duplicate `registry_for_repo` construction path stays private
   (`pub(crate)`). `run` builds the registry from `RunConfig` when
   `registry` is not set.

### D4 — stateless session store (REQ-SAG-0607)

1. `FileSessionStore { dir: PathBuf, ttl: Duration, clock: Arc<dyn Fn() -> SystemTime + Send + Sync> }`
   with `new(dir) -> Result<Self, JevError>`, `with_ttl`, `with_clock`,
   `save(&self, &StoredSession)`, `load(&self, &SessionId)`, and
   `ask(&self, id, follow_up: &str, registry: &BackendRegistry, log: &SharedSessionLog) -> Result<JudgeOutcome, JevError>`.
2. Delete `trait SessionStore`, `install_registry`, `record_outcome`, `open`,
   `load_raw` (merged into `load`), and the `session_log` constructor argument.
3. `run` writes the session **once**, at the end, and only when some
   participating backend has `capabilities().supports_session_ask`. The
   per-backend reload from disk is removed. a-10's atomic-write, fsync and 0700
   rules are kept.

### D11 — command producer contract (REQ-SAG-0312, REQ-SAG-0313)

1. `schema::check::InputSpec::Command` gains `path: Option<String>`. It is a
   repo-relative path validated by the same rules as `file` inputs, and it is
   substituted for `{path}` in argv using a-10's single-pass substitution.
   `{path}` appearing in argv while `path` is absent is `CONFIG_INVALID`.
2. When the command exits non-zero, `extract/command.rs` reads the last
   non-empty stderr line (bounded by the stderr cap). If that line parses as
   `ScAgentErrorResponse`, it re-raises `JevError::from_error_response`.
   Otherwise it keeps today's `ExtractionFailed`.
3. A unit test uses a fake producer script under `crates/sc-jev-check/tests/fixtures/`.
   The real-binary e2e test lands in r-3b.

### D13 — extractor and byte cap (REQ-SAG-0314)

1. Delete `pub trait Extractor`, `mod private { Sealed }`, and `extractor_for`.
   `extract_input` becomes a `match` over the four `InputSpec` kinds calling
   private functions.
2. Single owner of the request byte cap: each backend.
   `sc-jev-backends/src/validate.rs::MAX_REQUEST_BYTES` (96 000) and the
   matching `sc-jev` constant are reported through
   `capabilities().request_limit_bytes` and enforced in `validate_for_send`
   (`INPUT_TOO_LARGE` with `limit_bytes`). Delete `payload::refuse_over_cap`
   from the request path and remove `max_input_bytes` (200 000) from
   `PackagedConfig` and `assets/jev.toml`. `run` calls `validate_for_send` on
   **every** selected backend before the first `judge` (ADR-SAG-0004). Dry-run
   constructs no backends and applies no cap; it reports `payload_bytes`.
   Per-file read caps from a-10 stay as resource guards and are not the
   request cap.

### ADR text

Land the amendments to ADR-SAG-0019 (`RunOptions` → `RunConfig`) and
ADR-SAG-0022 (scripted registry via `RunConfig::registry`; the
`register_scripted_backends` signature is unchanged), remove the Extractor
sealing note under "Rust API patterns", and flip ADR-SAG-0024 to Active.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0620 | Only `prepare`/`run`/`discover_repo_root` are public run entry points (`test_public_run_surface` via `cargo public-api` diff or a grep test); `test_run_response_contract` pins the JSON; `sc-ai` and `sc-jev-py` produce byte-identical `data` for the same fixture |
| req:REQ-SAG-0607 | `SessionStore` trait and `install_registry` gone; `test_session_written_once_only_with_llm`; `test_ask_takes_registry` |
| req:REQ-SAG-0312 | `test_command_path_placeholder_from_check_toml`; `test_command_placeholder_without_path_is_config_invalid` |
| req:REQ-SAG-0313 | `test_command_stderr_error_response_reraised`; `test_command_stderr_unknown_code_is_extraction_failed` |
| req:REQ-SAG-0314 | No `refuse_over_cap` on the request path; no `max_input_bytes`; `test_payload_between_old_caps_refused_by_backend_before_send` |
| adr:ADR-SAG-0024 | Status Active; text matches signatures |
| adr:ADR-SAG-0022 | Amendment landed; scripted tests pass through `RunConfig::registry` |
| adr:ADR-SAG-0021 | Path precedence tests in `sc-ai` and `sc-jev-py` unchanged and green |
| boundary:jev-check | Regression: README parity rows, cross-impl digests, session 0600/expiry tests |

## Required validation

```bash
just lint
just test
cargo test -p sc-jev-check -- --nocapture
cargo test -p sc-jev-check -- test_run_response_contract --exact
cargo test -p sc-jev-check -- test_session_written_once_only_with_llm --exact
cargo test -p sc-jev-check -- test_command_path_placeholder_from_check_toml --exact
cargo test -p sc-jev-check -- test_command_stderr_error_response_reraised --exact
cargo test -p sc-jev-check -- test_payload_between_old_caps_refused_by_backend_before_send --exact
cargo test -p sc-ai -- --nocapture
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
cargo publish --dry-run --locked -p sc-jev-check
```

## This sprint does not close

Per-backend failure semantics, independent/chain modes, and digest-based
`equal` (r-2b); `sc-agent-compose` and the e2e producer test (r-3b); the HTTP
crate (r-3); D9 (r-5).
