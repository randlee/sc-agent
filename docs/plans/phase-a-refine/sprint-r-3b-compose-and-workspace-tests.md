---
id: r-3b
phase: r
status: planned
closure_type: integration
target_boundary: agent-compose
track: integration
wave: 4
size: XL
branch: sprint/r-3b-compose-and-workspace-tests
worktree: sprint/r-3b-compose-and-workspace-tests
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-2b, r-3, r-3c]
parallel_safe: []
vertical_rationale: "Integration sprint of the runtime track: creates sc-agent-compose and workspace-tests, moves both composition roots onto them, and closes every consumer-visible acceptance criterion of D2, D3, D7 and D12."
requirements:
  - REQ-SAG-0109
  - REQ-SAG-0604
  - REQ-SAG-0605
  - REQ-SAG-0712
  - REQ-SAG-0135
  - REQ-SAG-0711
  - REQ-SAG-0130
  - REQ-SAG-0313
  - REQ-SAG-0131
  - REQ-SAG-0132
  - REQ-SAG-0310
  - REQ-SAG-0600
  - REQ-SAG-0601
  - REQ-SAG-0603
  - REQ-SAG-0610
  - REQ-SAG-0611
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0702
  - REQ-SAG-0710
  - REQ-SAG-0800
  - REQ-SAG-0801
  - REQ-SAG-0802
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
  - NFR-SAG-0004
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0005  # compose calls prepare/run and never PayloadBuilder
  - ADR-SAG-0008  # r-2b amendment (Active after wave 3): Independent default, ChainContext explicit; ComposeInputs::mode and both consumers' flag/kwarg
  - ADR-SAG-0009
  - ADR-SAG-0010
  - ADR-SAG-0011
  - ADR-SAG-0013
  - ADR-SAG-0014
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0019  # prompt precedence: ComposeInputs::system_prompt_override and the clap port of --llm-system-prompt-file
  - ADR-SAG-0020
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0023
  - ADR-SAG-0024
  - ADR-SAG-0025
  - ADR-SAG-0026
  - ADR-SAG-0027
  - ADR-SAG-0028
  - ADR-SAG-0029
owned_paths:
  - "crates/sc-agent-compose/**"
  - "crates/workspace-tests/**"
  - "tests/jev_cross_impl/**"
  - "tests/fixtures/jev-python/**"
  - "tests/fixtures/jev/run-response/**"
  - "crates/sc-ai/**"
  - "crates/sc-jev-py/**"
  - "boundaries/sc-agent-compose/**"
  - "boundaries/workspace-tests/**"
  - "boundaries/sc-ai/**"
  - "boundaries/sc-jev-py/**"
  - "boundaries/sc-jev-check/**"
  - "boundaries/sc-agent-types/**"
  - "Cargo.toml"
  - "sc-lint.toml"
  - "clippy.toml#disallowed-methods std::process::Command::new entry"
  - "crates/sc-agent-types/src/git.rs#expect line only (item-level `#[expect(clippy::disallowed_methods, reason = ...)]` on the `git` spawn)"
  - "crates/sc-jev-check/src/extract/command.rs#expect line only (item-level expect on the user-argv spawn and on its test-module re-exec)"
  - "crates/sc-jev-check/src/extract/mod.rs#expect line only (item-level expect on `test_support::git_command`)"
  - "crates/*/tests/** and crates/*/src/**/tests.rs#item-level `#[expect(clippy::disallowed_methods, reason = ...)]` on the spawning helper only, where clippy flags `Command::new`"
  - ".github/workflows/sc-agent-ci.yml"
  - ".github/scripts/cargo-tree-gates.sh"
  - "release/**"
  - ".github/workflows/*-publish.yml"
  - ".github/workflows/release*.yml"
  - ".github/workflows/prerelease-archive.yml"
  - ".github/workflows/sc-publish-dry-run.yml"
  - "boundaries/README.md"
  - "docs/sc-agent-compose/**"
  - "docs/sc-ai/**"
  - "docs/sc-jev-py/**"
  - "docs/architecture.md#Phase A.1 crate graph"
  - "docs/architecture.md#ADR-SAG-0010 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0011 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0013 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0016 heading and amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0017 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0020 amendment (r-3b) (inserted as Proposed by r-3; r-3b flips it)"
  - "docs/architecture.md#ADR-SAG-0025 r-3b bullet (REQ-SAG-0604 consumer output), Status line (r-3b clause)"
  - "docs/architecture.md#ADR-SAG-0023 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0023 Status line (r-3b clause)"
  - "docs/architecture.md#ADR-SAG-0026 Status line and compose half"
  - "docs/architecture.md#ADR-SAG-0028 (incl. its Status line)"
  - "docs/architecture.md#ADR index row ADR-SAG-0016 (Title cell)"
  - "docs/architecture.md#ADR index row ADR-SAG-0023 (Status cell: compose pin)"
  - "docs/architecture.md#ADR index Amended-by marker (r-3b) in ADR-SAG-0010, 0011, 0013, 0016, 0017, 0020"
  - "docs/architecture.md#ADR index row ADR-SAG-0025 (Status cell: consumer-output bullet)"
  - "docs/architecture.md#ADR index row ADR-SAG-0026 (Status cell: compose half)"
  - "docs/architecture.md#ADR index row ADR-SAG-0028"
  - "docs/requirements.md#REQ-SAG-0109, 0135, 0604, 0605, 0712 rows; REQ-SAG-0130 (r-3b bullet), REQ-SAG-0711 amendments"
---

# Sprint r-3b — composition crate, workspace tests, consumers (D2 compose, D3 consumers, D7 tests, D12 sc-ai)

## Outcome

Composition-root logic lives once, in `sc-agent-compose`: the observability
adapter, path policy, repo discovery and `run`/`ask` wiring. `sc-ai` and
`sc-jev-py` become argv/PyO3 adapters over it (a future MCP server is a third
thin adapter), print partial failures identically, and expose
`--chain-context` / `chain_context`. Cross-crate proofs live in
`crates/workspace-tests`, including the first real end-to-end run of the
`sc-treesitter` producer through `check.toml`. `sc-ai` parses argv with clap.

## Closes findings

EXT-002, EXT-010, ARCHE-006, CR-001, ARCHP-002 (e2e), ARCHE-018, RBP-F012,
RBP-F013, QA-004, ARCHP-014, RBQA-F005, ARCHP-020, ARCH-005 (phase-end, A.1
graph); contributes ARCHE-002 (consumer flag).

## Deliverables

### D2 compose (ADR-SAG-0026 compose half, ADR-SAG-0016/0010/0011/0013 r-3b amendments)

1. New crate `crates/sc-agent-compose`, **published to crates.io as an
   internal-support crate** (owner decision (a), DECIDED 2026-11-06):
   `=`-pinned from `sc-ai` and `sc-jev-py` under the ADR-SAG-0023 lockstep
   rule, no semver promise of its own (README and crate rustdoc say so).
   `[lints] workspace = true`. Depends on `sc-jev-check`, `sc-agent-types`,
   `sc-observability =1.5.0`, `serde_json`. Consumer input: add it with `publish_order` directly after
   `sc-jev-check`, shift later crates, re-run `install.py` (r-3b owns the
   rendered files this wave). REQ-SAG-0130 r-3b bullet records the new
   published crate and order.
2. Contents:
   - `obs::ObservabilityAdapter`, moved from `sc-ai/src/observe.rs` and
     `sc-jev-py/src/logging.rs`, implementing `SessionLog` and mapping
     `EventError → LogRejected`; keeps a-10's one-logger-per-root and flush
     rules; one fallback action name and field order (QA-004). The
     adapter's tests (moved from both roots) are target `obs`
     (`cargo test -p sc-agent-compose --test obs`; REQ-SAG-0610 emission and
     REQ-SAG-0611 fields).
   - `paths::PathPolicy` — one plain data type with public fields
     `{ service: &'static str, log_root_flag: &'static str, log_root_env: &'static [&'static str], log_root_default: DefaultRoot, session_dir_env: &'static [&'static str], session_dir_default: SessionDefault }`
     with `resolve_log_root`, `resolve_session_dir`, `jsonl_path`, and presets
     `PathPolicy::cli()` and `PathPolicy::python()` encoding ADR-SAG-0021
     **per root, unchanged**. The two default types carry data, so a new
     consumer is not limited to the two presets (EXT-031):
     `enum DefaultRoot { UnderRepoRoot(PathBuf), UnderHome { home_relative: PathBuf, temp_fallback: PathBuf }, Fixed(PathBuf) }`
     (`cli()`: `UnderRepoRoot(".sc-ai")`; `python()`:
     `UnderHome { ".sc-agent/log", "sc-agent/log" }`, with the ADR-SAG-0021
     `HOME` → `USERPROFILE` → temp-dir order) and
     `enum SessionDefault { UnderLogRoot(PathBuf), UnderRepoRoot(PathBuf), Fixed(PathBuf) }`
     (`cli()`: `UnderRepoRoot(".sc-ai/sessions")`; `python()`:
     `UnderLogRoot("sessions")`). A new consumer (an MCP server) writes its
     own `PathPolicy` literal; it does not edit this crate (EXT-012).
     `test_custom_path_policy_resolves` (target `paths`) builds a third
     policy outside the presets, with `Fixed` defaults and its own
     environment variable, and asserts flag > environment > default.
   - `compose::ComposeInputs`: **private fields** (`cwd`, `policy`,
     `log_dir`, `sessions_dir`, `backends`, `rev`, `dry_run`,
     `system_prompt_override`, `mode`, `cancel`, `registry`), constructed only
     by `ComposeInputs::new(cwd: PathBuf, policy: PathPolicy)` with
     defaults (no overrides, `Independent`, never-cancelled token, production
     registry) plus one by-value setter per optional field
     (`log_dir(..)`, `sessions_dir(..)`, `backends(..)`, `rev(..)`,
     `dry_run(..)`, `system_prompt_override(..)`, `mode(..)`, `cancel(..)`,
     `registry(..)`). A new field is a new setter with a default, so adding
     one does not break `sc-ai`, `sc-jev-py` or an external caller (EXT-021).
     External consumers with their own composition needs call
     `sc_jev_check::{prepare, run}` directly; compose has no semver promise.
     **Type note** (EXT-033, README and ADR-SAG-0026): `run_from` and
     `ask_from` signatures name types owned by `sc-jev-check`
     (`CheckSource`, `RunResponse`, `JudgeOutcome` via `sc-agent-types`,
     `JevError`); a consumer depends on `sc-jev-check` and `sc-agent-types`
     directly, at the versions this compose release pins.
   - `compose::run_from(source: CheckSource, inputs: ComposeInputs) -> Result<RunResponse, JevError>`
     (calls `discover_repo_root`, builds `RunConfig`, `prepare`, `run`) and
     `compose::ask_from(session_id, follow_up, inputs) -> Result<JudgeOutcome, JevError>`
     (builds `FileSessionStore` + `BackendRegistry::from_env` unless a registry
     is injected).
   - **Warnings.** When the logger reports a flush failure (the Phase A
     CR-014 log-flush warning), `run_from` appends it with
     `RunResponse::with_warning` before returning, so both consumers emit it
     identically in `data.warnings`; neither consumer adds keys of its own
     besides `sc-ai`'s `with_readback`.
   - **Environment ownership** (ADR-SAG-0026): API-key environment and
     `.env` reading stays in `sc-jev-check` (`BackendRegistry::from_env` →
     `sc-agent-secrets`); log/session path environment variables are read
     only by `PathPolicy` in this crate. No other crate reads either set.
3. `sc-ai` and `sc-jev-py` delete their local adapters, path defaults,
   backend parsing (they use `BackendSet::from_str`) and response shaping
   (they print `Envelope::run(&response)`, plus `with_readback` in `sc-ai`).
   Their direct `sc-observability` dependency is removed; `sc-lint.toml`
   `sc-observability` record → `crates/sc-agent-compose` only (the
   `sc-observability-types` record was deleted by r-4). They keep their
   direct `sc-jev-check` edge for `CheckSource`, `Envelope`, `RunResponse`:
   compose does not re-export them (one path per item).
4. **Crate docs.** `docs/sc-agent-compose/requirements.md` (rows mapping to
   REQ-SAG-0604, 0605, 0711, ADR-SAG-0021 precedence) and
   `docs/sc-agent-compose/architecture.md` (adapter, `PathPolicy`,
   `ComposeInputs`, warnings, environment ownership, the EXT-033 type note).
   `docs/sc-ai/` and `docs/sc-jev-py/` gain the ADR-SAG-0027 consumer rule
   for stderr `schema_version` (EXT-030): a value other than `1` means
   "unknown error shape"; fall back to the exit code and the raw line.

### D3 consumers (REQ-SAG-0604, REQ-SAG-0605)

1. `sc-ai jev run --chain-context` sets `MultiBackendMode::ChainContext`;
   Python `run_check(..., chain_context: bool = False)` likewise. Both are
   pass-through.
2. Partial failure, both consumers through `RunResponse::is_partial()` /
   `first_failure()`: stdout prints the envelope with the successful answers;
   stderr prints the first failure's `ScAgentErrorResponse`; `sc-ai` exits 2.
   Python raises `JevError` carrying the code and a `.response` attribute
   holding the envelope `data` as a dict. All-failed stays a plain error
   (exit 2 / `JevError` without `.response`). `.pyi` and docs updated.
3. Tests: `test_cli_partial_failure_both_streams_exit_2`,
   `test_cli_chain_context_flag`, `test_cli_log_flush_warning_in_data`
   (`crates/sc-ai/tests/cli.rs`). Seam per the plan "Test seams"
   convention: `sc-ai` gains `test-util = ["sc-jev-check/test-util"]`; its
   `cli` target is declared `[[test]] name = "cli" required-features = ["test-util"]`
   and its validation lines pass `--features test-util` (no self
   dev-dependency: r-1's D0 spike result, `boundaries/README.md`
   "Dev-dependency and self edges"); only under that feature
   does the binary accept a hidden `--scripted-answers <file>` flag. The file
   is the r-2b D5 scripted-answers JSON (shared or per-backend form), passed
   unchanged to `register_scripted_backends` and injected through
   `ComposeInputs::registry`; the partial-failure test uses the per-backend
   form with one `failure` entry. The published binary has no such flag
   (`test_util_off`, `test_release_binary_rejects_scripted_answers`).
   **One library seam, one binary seam:** `sc_ai::execute_with_overrides`
   and `ExecuteOverrides` (a-10) are deleted. Every `sc-ai` test drives the
   built binary (with `--scripted-answers` where it needs backends); the
   session-expiry case it covered with an injected clock is covered by
   `sc-jev-check`'s session tests (`FileSessionStore::with_clock`), and the
   CLI keeps only the error-shape check, against a session file whose
   `created_at` is in the past.
   **Python** (ADR-SAG-0029 (b), unchanged): scripted injection is
   `sc_jev_py._testing.run_check_scripted(check_path, *, scripted_answers, ...)`,
   registered only under the `sc-jev-py` `test-util` feature; r-3b adds the
   same `chain_context` keyword to it, and `scripted_answers` takes the same
   r-2b D5 JSON. pytest `test_partial_failure_raises_with_response` (per-backend
   form) and `test_chain_context_kwarg` call it. The pytest wheel is built
   with `--features extension-module,test-util` (the maturin-feature
   exception in plan.md "Test seams"); the release wheel is built without
   `test-util` and has no `_testing` submodule (checked below).

### D7 workspace-tests (REQ-SAG-0109)

0. **Apply the r-1 spike result** (RBQA-F101; the spike ran in wave 1,
   r-1 D0, and is recorded in `boundaries/README.md` "Dev-dependency and
   self edges"). The recorded result is outcome 4, **dev edges counted**.
   Write `boundaries/workspace-tests/workspace-tests.toml` (plan.md "New
   boundary manifests") with its three workspace dev-dependencies
   (`sc-agent-types`, `sc-jev-check`, `sc-agent-compose`) in its dependency
   allowlist (`boundaries/README.md` case 1b accepts `allowed_dependencies`
   or `allowed_dev_dependencies`) and `facade = "workspace_tests"` (the
   spike showed `facade = ""` is rejected), and add `+workspace-tests` to
   `allowed_dependents` of `sc-agent-types`, `sc-jev-check` and
   `sc-agent-compose`. `sc-boundary` polices the workspace dev edges, so
   `cargo-tree-gates.sh` has no workspace-tests dev-set row. The same `boundaries/README.md`
   edit drops `sc-observability-types` from the `SCB-MANIFEST-004` row
   (record deleted by r-4).
1. `crates/workspace-tests` (`publish = false`; `[lints] workspace = true`;
   empty `src/lib.rs`; no normal dependencies; dev-deps exactly
   `sc-agent-types` (`features = ["git"]`), `sc-jev-check`,
   `sc-agent-compose`, `serde_json`, `tempfile` — **no `sc-agent-http`**,
   which no target links; no `pyo3` or `tree-sitter` in its graph). Manifest
   text in plan.md; ADR-SAG-0028.
2. Binaries are built with
   `std::process::Command::new(env!("CARGO")).args(["build", "--release", "-p", <pkg>, "--features", "cli", "--target-dir", <isolated>, "--message-format=json"])`
   and the executable path is read from the `compiler-artifact` message
   (stable Cargo, no `CARGO_BIN_EXE_*` across packages, no artifact
   dependencies). Helper in `crates/workspace-tests/tests/support/bin_path.rs`
   (a test-support module, so its `serde_json` use is a dev-dependency),
   included by each target with `mod support;`. `<isolated>` is always
   `$CARGO_TARGET_DIR/workspace-tests-bin`, and when `CARGO_TARGET_DIR` is
   unset, `<workspace root>/target/workspace-tests-bin`, where the workspace
   root is `concat!(env!("CARGO_MANIFEST_DIR"), "/../..")` canonicalized
   (`crates/workspace-tests` is two levels down; `cargo test` runs each
   target with the package directory as cwd, so a relative `target/` would
   land in `crates/workspace-tests/target`). It is never the directory `cargo test` uplifts
   `CARGO_BIN_EXE_sc-ai` into: that test binary is built with `test-util`
   on (`--features test-util`, or Cargo feature unification in a workspace
   test run), so sharing an uplift path would make the
   parity golden and `test_util_off` depend on test order (RBQA-F103 (b)).
   The helper therefore runs release-shaped binaries only. A fixed isolated
   directory is used instead of a tempdir per test so `sc-ai` and
   `sc-treesitter` are not rebuilt from scratch by every test; the isolation
   from `cargo test`'s artifacts is the same.
3. Targets:
   - `tests/jev_cross_impl.rs` (moved from `tests/jev_cross_impl/`; the
     `[[test]]` in `sc-jev-py/Cargo.toml` is deleted; uses only Rust crates
     and the committed Python reference bytes).
   - `tests/e2e_treesitter_producer.rs`: runs a `check.toml` with
     `kind = "command"`, `path = "src/sample.py"`, argv containing `{path}`,
     through `compose::run_from` with `dry_run`; asserts the subjects digest
     equals the committed `branches` fixture
     (`test_e2e_producer_digest_matches_fixture`), and that a bad language
     produces `LANGUAGE_UNAVAILABLE` re-raised as `ProducerError`
     (`test_e2e_producer_error_reraised`).
   - `tests/consumer_parity.rs`: `test_cli_and_python_data_match_golden` runs
     the built `sc-ai` binary on a dry-run fixture and asserts `data` equals
     `tests/fixtures/jev/run-response/dry_run_cli.json`; the pytest
     `test_run_data_matches_golden` asserts the Python result equals the same
     file (minus `readback`), so both consumers are pinned to one golden.
   - `tests/git_read_parity.rs`: `sc_agent_types::git::read_at_rev` on a temp
     repo matches `git cat-file blob` bytes for files `sc-jev-check` and the
     `sc-treesitter` binary read, and `--output=FILE` as `--rev` to the
     `sc-treesitter` binary creates no file.
   - `tests/source_scans.rs` (reads files, links nothing). **Needles are
     built from fragments** (for example `concat!("#[al", "low(")`), so the
     file never contains a literal it hunts and the `rg` gate needs no
     exclusion for `crates/workspace-tests` (CPR-M207, RBQA-F107); rustdoc
     prose says "allow attribute", never the literal. Attributes are read
     whole: each `#[…]` / `#![…]` from its `#` to the matching `]`, across
     lines, with whitespace removed, before any match. Targets (all scan
     every target under `crates/**/*.rs`: `src`, `tests`, `benches`,
     `examples`, `build.rs`):
     - `test_no_allow_attributes`: no attribute contains `allow(`, including
       inside `cfg_attr(…)`; no `expect(` names `warnings`, `clippy::all` or
       `clippy::restriction`. This is the one robust allow check (EXT-032,
       RBQA-F102 (a)): clippy's `allow_attributes` sees only compiled code,
       and the `rg` gate misses spaced and multi-line forms, so from r-3b on
       this target replaces the `rg` gate in sprint validation and in phase
       acceptance.
     - `test_disallowed_methods_expect_sites`: an `expect(` naming
       `clippy::disallowed_methods` (item- or crate-level, also inside
       `cfg_attr`) appears only in
       `crates/sc-agent-http/src/ureq_transport.rs` (the
       `ResolvedApiKey::as_str` read), `crates/sc-agent-types/src/git.rs` and
       `crates/sc-jev-check/src/extract/command.rs` (`Command::new`; the
       extractor spawns the configured argv), the `#[cfg(test)]`
       `test_support` module of `crates/sc-jev-check/src/extract/mod.rs`
       (a-10 :87-132; its `git_command` helper builds `git` fixture repos
       with an isolated config, which `sc_agent_types::git` does not offer;
       item-level expect on that helper; CPR-301 option 2), and in test files
       (`crates/*/tests/**`, `crates/*/src/**/tests.rs`) whose item-level
       expect has a reason naming a process spawn. A file of the last two
       kinds must not contain the token `ResolvedApiKey`, and a test file's
       expect must be item-level on the spawning helper (or on a named
       `support` module), never crate-level, so an expect cannot cover a
       secret read made through an inferred type. The type-aware half is clippy:
       `clippy.toml` `disallowed-methods` lists `std::process::Command::new`
       (added here, once r-2 and r-3c have moved every `git` caller to
       `sc_agent_types::git`) besides `ResolvedApiKey::as_str`, under
       `cargo clippy --workspace --all-targets -D warnings`. This replaces
       the text-match `test_git_spawn_only_in_types_git` and
       `test_disallowed_methods_expect_only_in_ureq_transport` of round 3
       (RBQA-F102 (d), RBQA-F003, RBQA-F006). r-3b adds the item-level
       expect to the spawning helper in each test file clippy flags.
       **Same-sprint rule (CPR-301):** every production and `test_support`
       expect line (`git.rs`, `command.rs`, `extract/mod.rs`) lands in r-3b
       together with the `clippy.toml` entry, never earlier: an `#[expect]`
       with no lint to fulfil is `unfulfilled_lint_expectations`, which
       `-D warnings` fails. Before merge r-3b runs
       `cargo clippy --workspace --all-targets -- -D warnings` with the new
       entry as a sweep, and the PR lists every site it fixed (file, item,
       reason).
     - `test_lint_mechanism_present` (RBQA-F102 (b), (c); RBQA-F103 (c)):
       `clippy.toml` lists both disallowed paths; root `Cargo.toml`
       `[workspace.lints.clippy]` sets `allow_attributes = "deny"` and
       `allow_attributes_without_reason = "deny"`; root `[workspace]` has
       `resolver = "2"` (the dev-only `test-util` seam depends on it: under
       resolver 1 a dev-dependency feature leaks into `cargo build`); every
       root `members` entry is `crates/<name>`; the `sc-lint.toml` clippy
       command that `just lint` runs contains `--workspace`,
       `--all-targets`, `-D` and `warnings`. Deleting any of them fails the
       scan.
     - `test_every_crate_inherits_workspace_lints`: every
       `crates/*/Cargo.toml` has `[lints] workspace = true` and no
       `[lints.*]` table of its own.
     - `test_env_reads_only_in_owners` (RBQA-F106 (d), ADR-SAG-0026
       environment ownership): `env::var(`, `env::var_os(`, `env::vars(`,
       `env::vars_os(` in `crates/*/src/**` appear only in
       `crates/sc-agent-secrets/src/**`,
       `crates/sc-jev-check/src/extract/sanitize.rs` and
       `crates/sc-jev-check/src/extract/command.rs` (the child-environment
       allowlist) and `crates/sc-agent-compose/src/paths.rs`; tests are
       exempt. It is a text scan; an aliased import evades it, and review
       covers that.
   - `tests/test_util_off.rs`:
     - `test_test_util_absent_from_normal_graph` runs
       `cargo tree --workspace -e normal,features` (a graph query, no test
       build, no dev edges) and fails if any workspace crate's `test-util`
       feature appears.
     - `test_release_binary_rejects_scripted_answers` (RBQA-F103 (a)):
       builds `sc-ai` through `bin_path` (release, isolated target dir) and
       runs `sc-ai jev run --scripted-answers x`; asserts exit 2 and one
       stderr `ScAgentErrorResponse` line for the unknown argument (clap
       error mapped by D12.1).
     Together with the release-wheel check below, these are the proof for
     the plan "Test seams" convention.

### D12 sc-ai clap (REQ-SAG-0712, REQ-SAG-0135, ADR-SAG-0010 r-3b amendment)

1. `#[derive(Parser)] struct Cli { #[command(subcommand)] cmd: Cmd }` with
   `Jev(JevCmd)`; `--rev` `conflicts_with = "request_json"`; values starting
   with `-` accepted via `allow_hyphen_values`; `--chain-context`.
   `try_parse_from`: `DisplayHelp`/`DisplayVersion` print and exit 0; every
   other error goes to stderr as `ScAgentErrorResponse` with the code current
   tests expect, exit 2. The a-10 edge cases are ported to
   `crates/sc-ai/tests/argv.rs`, declared in `crates/sc-ai/Cargo.toml` as
   `[[test]] name = "argv" required-features = ["cli"]` (it runs the
   `cli`-gated binary; r-4 already gates `cli`). Delete the hand-rolled
   parser and its flag tables. `--llm-system-prompt-file` maps to
   `ComposeInputs::system_prompt_override` (ADR-SAG-0019 precedence, item 1);
   new `test_cli_system_prompt_file_overrides_check_toml` in
   `crates/sc-ai/tests/cli.rs` runs a check whose `check.toml` names one
   prompt file and the flag another (scripted Haiku backend,
   `--features test-util`), and asserts the saved session document records
   the flag's prompt text.
2. CI (`sc-agent-ci.yml` + `cargo-tree-gates.sh`): add every row of the
   plan.md `cargo tree` gate table not yet scripted (clap for both libraries,
   ureq, pyo3, tree-sitter, sc-observability, the `sc-treesitter` →
   `sc-agent-types` feature row r-3c checks locally, the `sc-agent-http` →
   `sc-agent-types` `secret`-only feature row r-3 checks locally, the
   `test-util` row, the duplicate-version rows for `sc-ai` and `sc-jev-py`,
   and no `workspace-tests` dev-set row, since `sc-boundary` counts dev
   edges (D7.0)), each run with
   default features **and** `--no-default-features` except *(lib)* rows, with
   word-bounded patterns; the public-API check in the form r-3 proved with
   its canary (`cargo doc -p sc-agent-http`, with deps, then the `ureq/<kind>.`
   href / `ureq::` title grep); isolated per-library builds
   (`cargo build -p <lib> --lib` for each library crate, one invocation per
   crate so feature unification cannot hide a seam); the release-wheel check
   (default `maturin build`, then import it and assert there is no
   `_testing` submodule); a `workspace-tests` job; and a re-run of
   `check-exact-pins.sh` now that the `=` compose pin exists (NFR-SAG-0004
   regression). These gates run on plain `cargo build` / `cargo tree`, never
   on a test build, so a dev-only feature unified by `cargo test` cannot
   make them pass (RBP minor 3).

### Manifests

Root `Cargo.toml` members `sc-agent-compose`, `workspace-tests`. Boundary
manifests: `sc-agent-compose` and `workspace-tests` (full text in plan.md;
dev edges counted, D7.0); `sc-ai`, `sc-jev-py` `allowed_dependencies
-sc-observability +sc-agent-compose` with forbidden `sc-observability`,
`ureq`, and `sc-ai` `+clap`, `+mimalloc` (optional; r-4 recorded this
deferral); `sc-jev-check` and `sc-agent-types` `allowed_dependents
+sc-agent-compose +workspace-tests`; compose's `allowed_dependents` is
`sc-ai`, `sc-jev-py`, `workspace-tests`.
`sc-agent-compose` and `workspace-tests` both have `[lints] workspace = true`.
`crates/sc-ai/Cargo.toml` gains `[[test]] name = "argv" required-features = ["cli"]`
(D12.1).

### ADR text

**Wave-4 pre-flight** (plan review 5, pre-screen r-3b). Before
development starts, confirm that r-3 landed the ADR-SAG-0020 "Amendment
(Phase A.1, r-3b)" Proposed subsection (the `source_scans` bullet below) and
the "A.1 subsection (Proposed, r-3b)" marker in the ADR-SAG-0020 index row;
if not, r-3b inserts both, verbatim as below, before flipping them.

ADR-SAG-0026 compose half → Active and its ADR-level Status → Active;
ADR-SAG-0016 r-3b amendment (adapter in published `sc-agent-compose`;
supersedes "do not publish a separate observability crate" for that crate)
→ Active, and ADR-SAG-0016 is retitled "v2 shim in sc-agent-types; adapter
in sc-agent-compose" in its heading and index title cell; ADR-SAG-0017 r-3b
amendment (published library set + publish order gains `sc-agent-compose`)
→ Active; ADR-SAG-0023 r-3b amendment subsection (compose pin in force) →
Active, and the ADR-SAG-0023 Status line becomes "Active (Phase A.1) —
landed in r-4; its r-3b subsection landed in r-3b"; ADR-SAG-0028 Status
line "Active (Phase A.1) — landed in r-3b" and every bullet label
"(Active, r-3b)"; ADR-SAG-0028 (`workspace-tests`: purpose, dev-only edges, the
built-binary contract, forbidden edges, spike outcome) → Active, with its
index row; ADR-SAG-0010, ADR-SAG-0011, ADR-SAG-0013 r-3b amendments →
Active; A.1 crate graph section added.

**ADR-SAG-0020 r-3b bullet** (plan review 4, pre-screen r-3 fix 1). r-3
moves the `source_scans` clause out of its Active ADR-SAG-0020 r-3 bullet
and inserts it as a Proposed "Amendment (Phase A.1, r-3b)" subsection with
exactly this bullet; r-3b flips the subsection, its bullet label and the
"(Proposed, r-3b)" marker in the ADR-SAG-0020 index row to Active:

> - (Proposed, r-3b) `workspace-tests --test source_scans` is the
>   cross-crate proof of the ADR-SAG-0020 r-3 read-site convention:
>   `test_disallowed_methods_expect_sites` fails if an `expect` naming
>   `clippy::disallowed_methods` (item- or crate-level, also inside
>   `cfg_attr`) appears in any file of any target (`src`, `tests`,
>   `benches`, `examples`, `build.rs`) other than
>   `crates/sc-agent-http/src/ureq_transport.rs` and the process-spawn sites
>   ADR-SAG-0028 lists, and a spawn-site file must not contain
>   `ResolvedApiKey`; `test_every_crate_inherits_workspace_lints` fails if a
>   crate stops inheriting the workspace lints or adds a per-crate
>   `[lints.*]` override. From r-3b this replaces r-3's local `rg` check.

**ADR-SAG-0025 consumer-output bullet.** r-2b split bullet 2 and left the
"(Proposed, r-3b) REQ-SAG-0604 consumer output" bullet Proposed; r-3b flips
that bullet to Active, sets the ADR-SAG-0025 Status line to "Active" and the
index row Status cell to "Active" (D3.2 delivers it).

**ADR-SAG-0028 "Dev-only edges" bullet states the recorded outcome** (plan
review 5, ARCH-R3B-003; D0 outcome 4 of r-1, `boundaries/README.md`). Replace
the conditional sentences "Whether `sc-boundary` checks dev edges is settled
in wave 1 by r-1's spike and recorded in `boundaries/README.md`; if it does
not, `cargo-tree-gates.sh` polices the exact dev set." with:

> `sc-boundary` counts dev-only workspace edges (r-1's D0 spike, outcome 4,
> `boundaries/README.md` "Dev-dependency and self edges"): the
> `workspace-tests` manifest lists `sc-agent-types`, `sc-jev-check` and
> `sc-agent-compose` in its dependency allowlist, and each of those three
> lists `workspace-tests` in `allowed_dependents`. There is no
> `cargo-tree-gates.sh` dev-set row. The third-party dev-dependencies
> (`serde_json`, `tempfile`) are not checked by `sc-boundary`.

**ADR-SAG-0028 self dev-dependency sentence reworded** (D0 outcome 4 of
r-1, `boundaries/README.md`). In the "Built-binary contract" bullet,
replace "which has `test-util` on through its self dev-dependency" with
"which is built with `test-util` on (`--features test-util`, or Cargo
feature unification in a workspace test run; there is no self
dev-dependency; `boundaries/README.md`, section Dev-dependency and self edges)".

**Markers r-3b flips and markers it must not touch.** r-3b flips only the
"(Proposed, r-3b)" markers in index rows 0010, 0011, 0013, 0016, 0017,
0020 and 0023 (compose pin), the compose-half parts of the ADR-SAG-0026
row, the ADR-SAG-0025 Status cell and the ADR-SAG-0028 row, plus the
ADR-SAG-0023 and ADR-SAG-0028 Status lines named above. In the
ADR-SAG-0017 cell ("Amended by ADR-SAG-0023 (Proposed, r-4); ADR-SAG-0026
(Proposed, r-3), (Proposed, r-3b)") it flips only the last "(Proposed,
r-3b)"; the r-4 and r-3 markers there belong to r-4 and r-3, which have
already flipped them (wave 1 and wave 2), and r-3b must not rewrite them.
The same holds for the r-4 and r-1 markers in row 0016. REQ-SAG-0711 amended in
`docs/requirements.md` **and** the crate-level row in
`docs/sc-ai/requirements.md` (which still says
`sc-observability = "1.5.0"` + adapter): "`sc-ai` wires the logger through
`sc_agent_compose::obs`". The REQ-SAG-0701 crate-level row in
`docs/sc-ai/requirements.md` gains "one `ScAgentErrorResponse` line per
NFR-SAG-0001 (amended in r-1)". `workspace-tests` gets no `docs/` directory
(EXT-032): `crates/workspace-tests/README.md` holds one paragraph per target
(what it proves, how binaries are built, the scan rules) and points at
ADR-SAG-0028; REQ-SAG-0109 maps to the targets in the acceptance table
below.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0109 | `cargo test -p workspace-tests` runs the six targets above; `sc-jev-py/Cargo.toml` has no `[[test]]`; `cargo tree -p workspace-tests -e normal,dev` has no `pyo3`/`tree-sitter`; the manifests follow D0 outcome 4 (dev edges counted): `workspace-tests` is in `allowed_dependents` of `sc-agent-types`, `sc-jev-check` and `sc-agent-compose`, and `just lint` reports no `SCB-DEPENDENCY-001`/`-002`/`-004` |
| req:REQ-SAG-0711 (amended) | `cargo test -p sc-agent-compose --test obs` green; `grep -q 'sc_agent_compose::obs' docs/sc-ai/requirements.md` and the root row says the same |
| req:REQ-SAG-0130 (amended, r-3b bullet) | `release/sc-publish-consumer-input.json` lists `sc-agent-compose` directly after `sc-jev-check`; the `cargo publish --dry-run` below packages it in that position and exits 0 |
| req:REQ-SAG-0604 | `test_partial_failure_keeps_answers` (r-2b) still green; `test_cli_partial_failure_both_streams_exit_2`; pytest `test_partial_failure_raises_with_response` |
| req:REQ-SAG-0605 | `test_cli_chain_context_flag`; pytest `test_chain_context_kwarg`; independent is the default in both |
| req:REQ-SAG-0712 | No hand-rolled argv tables in either binary; `sc-ai` and `sc-treesitter` `tests/argv.rs` green |
| req:REQ-SAG-0135 | `cargo-tree-gates.sh` clap rows pass in CI for both libraries |
| adr:ADR-SAG-0026 | Compose half and ADR-level Status Active; `sc-ai`/`sc-jev-py` contain no adapter or path-default code (the moved files are deleted); `ComposeInputs` has no public field |
| adr:ADR-SAG-0028 | Status line and index row Active; the "Dev-only edges" bullet reads as above (no conditional; dev edges counted) and matches the four manifests |
| adr:ADR-SAG-0023 | r-3b subsection Active; Status line reads as above; `bash .github/scripts/check-exact-pins.sh` green with the compose pin |
| adr:ADR-SAG-0008 | Independent is the default in `ComposeInputs::new`, `sc-ai jev run` and `sc_jev_py.run_check`; `ChainContext` only on request (`test_cli_chain_context_flag`, pytest `test_chain_context_kwarg`; `equal_input.mode` in both outputs matches the request) |
| adr:ADR-SAG-0019 | Prompt precedence unchanged: `--llm-system-prompt-file` (clap) reaches `ComposeInputs::system_prompt_override` and wins over `check.toml` and the packaged default (`test_cli_system_prompt_file_overrides_check_toml`, D12.1; regression `cargo test -p sc-jev-check --lib -- run::prompt::`); Jev-only runs still need no prompt file |
| adr:ADR-SAG-0005 | Compose and both roots call `sc_jev_check::{prepare, run}` only and never `PayloadBuilder`: `! rg -n 'PayloadBuilder' crates/sc-agent-compose/src crates/sc-ai/src crates/sc-jev-py/src` |
| adr:ADR-SAG-0016, 0017, 0010, 0011, 0013, 0023 | r-3b subsections Active; ADR-SAG-0016 retitled; in rows 0016 and 0017 only the r-3b markers changed |
| adr:ADR-SAG-0020 | r-3b subsection Active with the bullet above; index row marker "(Active, r-3b)"; `cargo test -p workspace-tests --test source_scans -- test_disallowed_methods_expect_sites --exact` and `-- test_every_crate_inherits_workspace_lints --exact` print `1 passed` |
| adr:ADR-SAG-0025 | consumer-output bullet, Status line and index row Status cell Active; `test_cli_partial_failure_both_streams_exit_2`; pytest `test_partial_failure_raises_with_response` |
| adr:ADR-SAG-0003, 0009, 0027 | `.env` and API-key environment reads stay in `sc-jev-check` (`BackendRegistry::from_env`; 0003, `test_env_reads_only_in_owners`); compose builds `BackendRegistry::from_env` unless `ComposeInputs::registry` is set and adds no id dispatch (0009); consumer errors keep the r-1 `ErrorCode` mapping and NFR-SAG-0001 field rule in `sc-ai` and `sc-jev-py` (0027, `test_error_response_stderr_golden`, `test_cli_partial_failure_both_streams_exit_2`) |
| adr:ADR-SAG-0021 | `test_cli_path_policy_precedence`, `test_python_path_policy_precedence` in compose reproduce every existing precedence case; `test_custom_path_policy_resolves` (EXT-031) |
| adr:ADR-SAG-0029 | (b) holds: pytest reaches scripted backends only through `sc_jev_py._testing.run_check_scripted`; the release wheel has no `_testing` submodule (check below) |
| boundary:agent-compose | Manifest enforced (`just lint` output attached); `cargo tree` gates for compose pass |
| boundary:sc-ai-cli | Regression: REQ-SAG-0700–0711 tests green (REQ-SAG-0702 readback: `cargo test -p sc-ai --features test-util --test cli`); `test_cli_and_python_data_match_golden`; `test_cli_log_flush_warning_in_data` |
| boundary:workspace | `source_scans` and `test_util_off` targets green (incl. `test_release_binary_rejects_scripted_answers`); isolated library builds green; release wheel has no `_testing`; duplicate-version rows pass for `sc-ai` and `sc-jev-py` |
| boundary:regressions | REQ-SAG-0310 `cargo test -p sc-treesitter --test cli` and `e2e_treesitter_producer`; REQ-SAG-0313 `test_e2e_producer_error_reraised`; REQ-SAG-0600/0601 `cargo test -p sc-jev-check --features test-util --test sessions_exact` and `--test run_response`; REQ-SAG-0603 `test_cli_partial_failure_both_streams_exit_2` (two backends in one run); REQ-SAG-0610/0611 `cargo test -p sc-agent-compose --test obs`; NFR-SAG-0001 `cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact`; NFR-SAG-0002 `cargo test -p sc-jev-backends --test llm` and `cargo test -p sc-ai --features test-util --test cli`; NFR-SAG-0003 `cargo test -p sc-agent-compose --test paths` (default and configured session/log roots); NFR-SAG-0004 `bash .github/scripts/check-exact-pins.sh`; REQ-SAG-0800/0801/0802 maturin build + pytest below; REQ-SAG-0131/0132 `bash .github/scripts/run-consumer-preflight.sh` and the publish dry-run with `sc-agent-compose` in order |

## Required validation

```bash
just lint
just test
cargo test -p workspace-tests --test jev_cross_impl
cargo test -p workspace-tests --test e2e_treesitter_producer -- test_e2e_producer_digest_matches_fixture --exact
cargo test -p workspace-tests --test e2e_treesitter_producer -- test_e2e_producer_error_reraised --exact
cargo test -p workspace-tests --test consumer_parity -- test_cli_and_python_data_match_golden --exact
cargo test -p workspace-tests --test git_read_parity
cargo test -p workspace-tests --test source_scans -- test_no_allow_attributes --exact
cargo test -p workspace-tests --test source_scans -- test_disallowed_methods_expect_sites --exact
cargo test -p workspace-tests --test source_scans -- test_lint_mechanism_present --exact
cargo test -p workspace-tests --test source_scans -- test_every_crate_inherits_workspace_lints --exact
cargo test -p workspace-tests --test source_scans -- test_env_reads_only_in_owners --exact
cargo test -p workspace-tests --test test_util_off -- test_test_util_absent_from_normal_graph --exact
cargo test -p workspace-tests --test test_util_off -- test_release_binary_rejects_scripted_answers --exact
cargo clippy --workspace --all-targets -- -D warnings
! rg -n 'PayloadBuilder' crates/sc-agent-compose/src crates/sc-ai/src crates/sc-jev-py/src
cargo test -p sc-ai --features test-util --test cli -- test_cli_system_prompt_file_overrides_check_toml --exact
cargo test -p sc-jev-check --lib -- run::prompt::
cargo test -p sc-agent-compose --test obs
cargo test -p sc-jev-check --features test-util --test sessions_exact
cargo test -p sc-treesitter --test cli
cargo test -p sc-agent-compose --test paths -- test_cli_path_policy_precedence --exact
cargo test -p sc-agent-compose --test paths -- test_python_path_policy_precedence --exact
cargo test -p sc-agent-compose --test paths -- test_custom_path_policy_resolves --exact
cargo test -p sc-agent-compose --test paths
cargo test -p sc-agent-types --test errors -- test_error_response_stderr_golden --exact
cargo test -p sc-jev-backends --test llm
cargo test -p sc-ai --test argv
cargo test -p sc-ai --features test-util --test cli -- test_cli_partial_failure_both_streams_exit_2 --exact
cargo test -p sc-ai --features test-util --test cli -- test_cli_chain_context_flag --exact
cargo test -p sc-ai --features test-util --test cli -- test_cli_log_flush_warning_in_data --exact
cargo test -p sc-treesitter --test argv
cargo build -p sc-ai --lib --no-default-features
cargo build -p sc-treesitter --lib --no-default-features
for p in sc-agent-types sc-agent-secrets sc-agent-http sc-jev sc-jev-backends sc-jev-check sc-agent-compose; do cargo build -p "$p" --lib || exit 1; done
cargo doc -p sc-agent-http && ! grep -rlE 'ureq/(struct|enum|trait|type|fn|union)\.|title="(struct|enum|trait|type|fn|union) ureq::' target/doc/sc_agent_http/
test -z "$(cargo tree -p sc-ai -e normal -d | grep -E '(^|[^[:alnum:]_-])(sc-jev-check|sc-agent-types|sc-agent-compose) v[0-9]')"
test -z "$(cargo tree -p sc-jev-py -e normal -d | grep -E '(^|[^[:alnum:]_-])(sc-jev-check|sc-agent-types|sc-agent-compose) v[0-9]')"
bash .github/scripts/cargo-tree-gates.sh
bash .github/scripts/check-exact-pins.sh
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --isolated --no-project --with "$(ls /tmp/sc-agent-wheels/sc_jev_py-*.whl | head -n 1)" python -c 'import sc_jev_py, sys; sys.exit(1 if hasattr(sc_jev_py, "_testing") else 0)'
uv run --directory tests/fixtures/jev-python pytest -q
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

## This sprint does not close

MCP server (Phase B); removal of the v2 shim (`agt-obs-v2-shim-removal`);
any real crates.io publish (plan gate (c)).
