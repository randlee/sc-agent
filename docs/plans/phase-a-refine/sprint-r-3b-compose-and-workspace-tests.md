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
  - REQ-SAG-0700
  - REQ-SAG-0701
  - REQ-SAG-0710
  - NFR-SAG-0001
  - NFR-SAG-0002
  - NFR-SAG-0003
  - NFR-SAG-0004
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0010
  - ADR-SAG-0011
  - ADR-SAG-0013
  - ADR-SAG-0014
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0021
  - ADR-SAG-0022
  - ADR-SAG-0023
  - ADR-SAG-0024
  - ADR-SAG-0025
  - ADR-SAG-0026
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
  - ".github/workflows/sc-agent-ci.yml"
  - ".github/scripts/cargo-tree-gates.sh"
  - "release/**"
  - ".github/workflows/*-publish.yml"
  - ".github/workflows/release*.yml"
  - ".github/workflows/prerelease-archive.yml"
  - ".github/workflows/sc-publish-dry-run.yml"
  - "docs/sc-agent-compose/**"
  - "docs/sc-ai/**"
  - "docs/sc-jev-py/**"
  - "docs/architecture.md#Phase A.1 crate graph"
  - "docs/architecture.md#ADR-SAG-0010 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0011 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0013 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0016 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0017 amendment (r-3b)"
  - "docs/architecture.md#ADR-SAG-0023 compose-pin bullet"
  - "docs/architecture.md#ADR-SAG-0026 compose half"
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
   Depends on `sc-jev-check`, `sc-agent-types`, `sc-observability =1.5.0`,
   `serde_json`. Consumer input: add it with `publish_order` directly after
   `sc-jev-check`, shift later crates, re-run `install.py` (r-3b owns the
   rendered files this wave). REQ-SAG-0130 r-3b bullet records the new
   published crate and order.
2. Contents:
   - `obs::ObservabilityAdapter`, moved from `sc-ai/src/observe.rs` and
     `sc-jev-py/src/logging.rs`, implementing `SessionLog` and mapping
     `EventError → LogRejected`; keeps a-10's one-logger-per-root and flush
     rules; one fallback action name and field order (QA-004).
   - `paths::PathPolicy` — one data type
     `{ service: &'static str, log_root_flag, log_root_env: &'static [&'static str], log_root_default: DefaultRoot, session_dir_env: &'static [&'static str], session_dir_default: SessionDefault }`
     with `resolve_log_root`, `resolve_session_dir`, `jsonl_path`, and presets
     `PathPolicy::cli()` and `PathPolicy::python()` encoding ADR-SAG-0021
     **per root, unchanged**. A new consumer builds its own `PathPolicy`
     value; it does not edit this crate (EXT-012).
   - `compose::ComposeInputs { cwd: PathBuf, policy: PathPolicy, log_dir: Option<PathBuf>, sessions_dir: Option<PathBuf>, backends: Option<BackendSet>, rev: Option<Rev>, dry_run: bool, system_prompt_override: Option<PathBuf>, mode: MultiBackendMode, cancel: CancelToken, registry: Option<Arc<BackendRegistry>> }`
     (builder; private fields).
   - `compose::run_from(source: CheckSource, inputs: ComposeInputs) -> Result<RunResponse, JevError>`
     (calls `discover_repo_root`, builds `RunConfig`, `prepare`, `run`) and
     `compose::ask_from(session_id, follow_up, inputs) -> Result<JudgeOutcome, JevError>`
     (builds `FileSessionStore` + `BackendRegistry::from_env` unless a registry
     is injected).
3. `sc-ai` and `sc-jev-py` delete their local adapters, path defaults,
   backend parsing (they use `BackendSet::from_str`) and response shaping
   (they print `Envelope::run(&response)`, plus `with_readback` in `sc-ai`).
   Their direct `sc-observability` dependency is removed; `sc-lint.toml`
   `sc-observability*` record → `crates/sc-agent-compose` only.

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
   `test_cli_chain_context_flag` (`crates/sc-ai/tests/cli.rs`, scripted
   backends via `sc-ai`'s `test-util` dev path); pytest
   `test_partial_failure_raises_with_response`, `test_chain_context_kwarg`.

### D7 workspace-tests (REQ-SAG-0109)

1. `crates/workspace-tests` (`publish = false`; no normal dependencies;
   dev-deps on `sc-agent-types`, `sc-agent-http`, `sc-jev-check`,
   `sc-agent-compose`; no `pyo3` or `tree-sitter` in its graph). Manifest
   text in plan.md.
2. Binaries are built with
   `std::process::Command::new(env!("CARGO")).args(["build", "-p", <pkg>, "--features", "cli", "--message-format=json"])`
   and the executable path is read from the `compiler-artifact` message
   (stable Cargo, no `CARGO_BIN_EXE_*` across packages, no artifact
   dependencies). Helper in `crates/workspace-tests/src/bin_path.rs`.
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

### D12 sc-ai clap (REQ-SAG-0712, REQ-SAG-0135, ADR-SAG-0010 r-3b amendment)

1. `#[derive(Parser)] struct Cli { #[command(subcommand)] cmd: Cmd }` with
   `Jev(JevCmd)`; `--rev` `conflicts_with = "request_json"`; values starting
   with `-` accepted via `allow_hyphen_values`; `--chain-context`.
   `try_parse_from`: `DisplayHelp`/`DisplayVersion` print and exit 0; every
   other error goes to stderr as `ScAgentErrorResponse` with the code current
   tests expect, exit 2. The a-10 edge cases are ported to
   `crates/sc-ai/tests/argv.rs`. Delete the hand-rolled parser and its flag
   tables.
2. CI (`sc-agent-ci.yml` + `cargo-tree-gates.sh`): add every row of the
   plan.md `cargo tree` gate table not yet scripted (clap for both libraries,
   ureq, pyo3, tree-sitter, sc-observability), a `workspace-tests` job, and a
   re-run of `check-exact-pins.sh` now that the `=` compose pin exists
   (NFR-SAG-0004 regression).

### Manifests

Root `Cargo.toml` members `sc-agent-compose`, `workspace-tests`. Boundary
manifests: `sc-agent-compose` and `workspace-tests` (full text in plan.md);
`sc-ai`, `sc-jev-py` `allowed_dependencies -sc-observability +sc-agent-compose`
with forbidden `sc-observability`, `ureq`; `sc-jev-check` and
`sc-agent-types` `allowed_dependents +sc-agent-compose, +workspace-tests`.

### ADR text

ADR-SAG-0026 compose half → Active; ADR-SAG-0016 r-3b amendment (adapter in
published `sc-agent-compose`; supersedes "do not publish a separate
observability crate" for that crate) → Active; ADR-SAG-0017 r-3b amendment
(published library set + publish order gains `sc-agent-compose`) → Active;
ADR-SAG-0023 compose-pin bullet → Active; ADR-SAG-0010, ADR-SAG-0011,
ADR-SAG-0013 r-3b amendments → Active; A.1 crate graph section added.
REQ-SAG-0711 amended: "`sc-ai` wires the logger through
`sc_agent_compose::obs`".

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0109 | `cargo test -p workspace-tests` runs the four targets above; `sc-jev-py/Cargo.toml` has no `[[test]]`; `cargo tree -p workspace-tests -e normal,dev` has no `pyo3`/`tree-sitter` |
| req:REQ-SAG-0604 | `test_partial_failure_keeps_answers` (r-2b) still green; `test_cli_partial_failure_both_streams_exit_2`; pytest `test_partial_failure_raises_with_response` |
| req:REQ-SAG-0605 | `test_cli_chain_context_flag`; pytest `test_chain_context_kwarg`; independent is the default in both |
| req:REQ-SAG-0712 | No hand-rolled argv tables in either binary; `sc-ai` and `sc-treesitter` `tests/argv.rs` green |
| req:REQ-SAG-0135 | `cargo-tree-gates.sh` clap rows pass in CI for both libraries |
| adr:ADR-SAG-0026 | Compose half Active; `sc-ai`/`sc-jev-py` contain no adapter or path-default code (the moved files are deleted) |
| adr:ADR-SAG-0016, 0017, 0010, 0011, 0013, 0023 | r-3b subsections / bullet Active |
| adr:ADR-SAG-0021 | `test_cli_path_policy_precedence`, `test_python_path_policy_precedence` in compose reproduce every existing precedence case |
| boundary:agent-compose | Manifest enforced (`just lint` output attached); `cargo tree` gates for compose pass |
| boundary:sc-ai-cli | Regression: REQ-SAG-0700–0711 tests green; `test_cli_and_python_data_match_golden` |

## Required validation

```bash
just lint
just test
cargo test -p workspace-tests --test jev_cross_impl
cargo test -p workspace-tests --test e2e_treesitter_producer -- test_e2e_producer_digest_matches_fixture --exact
cargo test -p workspace-tests --test e2e_treesitter_producer -- test_e2e_producer_error_reraised --exact
cargo test -p workspace-tests --test consumer_parity -- test_cli_and_python_data_match_golden --exact
cargo test -p workspace-tests --test git_read_parity
cargo test -p sc-agent-compose --test paths -- test_cli_path_policy_precedence --exact
cargo test -p sc-agent-compose --test paths -- test_python_path_policy_precedence --exact
cargo test -p sc-ai --test argv
cargo test -p sc-ai --test cli -- test_cli_partial_failure_both_streams_exit_2 --exact
cargo test -p sc-ai --test cli -- test_cli_chain_context_flag --exact
cargo test -p sc-treesitter --test argv
cargo build -p sc-ai --lib --no-default-features
cargo build -p sc-treesitter --lib --no-default-features
bash .github/scripts/cargo-tree-gates.sh
bash .github/scripts/check-exact-pins.sh
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

## This sprint does not close

MCP server (Phase B); removal of the v2 shim (`agt-obs-v2-shim-removal`);
any real crates.io publish (plan gate (c)).
