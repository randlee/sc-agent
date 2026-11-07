---
id: r-3b
phase: r
status: planned
closure_type: integration
target_boundary: agent-compose
track: integration
wave: 3
branch: sprint/r-3b-compose-and-workspace-tests
worktree: sprint/r-3b-compose-and-workspace-tests
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [r-2, r-3, r-4]
parallel_safe: [r-2b]
requirements:
  - REQ-SAG-0109
  - REQ-SAG-0712
  - REQ-SAG-0711
  - REQ-SAG-0313
adrs:
  - ADR-SAG-0010
  - ADR-SAG-0016
  - ADR-SAG-0021
  - ADR-SAG-0026
owned_paths:
  - "crates/sc-agent-compose/**"
  - "crates/workspace-tests/**"
  - "tests/jev_cross_impl/**"
  - "boundaries/sc-agent-compose/**"
  - "boundaries/sc-ai/**"
  - "boundaries/sc-jev-py/**"
  - "boundaries/sc-treesitter/**"
  - "crates/sc-ai/Cargo.toml"
  - "crates/sc-ai/src/main.rs"
  - "crates/sc-ai/src/lib.rs"
  - "crates/sc-ai/src/command.rs"
  - "crates/sc-ai/src/observe.rs"
  - "crates/sc-ai/tests/argv.rs"
  - "crates/sc-jev-py/Cargo.toml"
  - "crates/sc-jev-py/src/logging.rs"
  - "crates/sc-jev-py/src/lib.rs"
  - "crates/sc-treesitter/Cargo.toml"
  - "crates/sc-treesitter/src/main.rs"
  - "crates/sc-treesitter/tests/argv.rs"
  - "Cargo.toml"
  - "release/sc-publish-consumer-input.json"
  - "docs/sc-agent-compose/**"
---

# Sprint r-3b — composition crate, workspace tests, clap (D2 compose, D7 tests, D12)

## Outcome

Composition-root logic lives once, in `sc-agent-compose`: the observability
adapter, path policy and repo discovery wiring. `sc-ai` and `sc-jev-py` become
argv/PyO3 adapters over it, and a future MCP server is a third thin adapter.
Cross-crate proofs live in `crates/workspace-tests`, including the first real
end-to-end run of the `sc-treesitter` producer through `check.toml`. Both
binaries parse argv with clap derive.

## Closes findings

EXT-002 (compose part), RBQA-F005, ARCHP-014, RBP-F013, ARCHP-002 (e2e proof),
CR-001 (e2e proof), ARCHE-018, EXT-010, RBP-F012, ARCHP-011 (argv part).

## Deliverables

### D2 compose (ADR-SAG-0026, ADR-SAG-0016 amendment, ADR-SAG-0010 amendment)

1. New crate `crates/sc-agent-compose`, **published as an internal-support
   crate** (see plan "Underspecified decisions": `sc-ai` is on crates.io, so its
   dependencies must be too). `=`-pinned from `sc-ai` and `sc-jev-py` under the
   ADR-SAG-0023 lockstep rule. README and rustdoc state "no stability promise
   outside this workspace". It depends on `sc-jev-check`, `sc-agent-types`, and
   `sc-observability =1.5.0`.
2. Contents:
   - `obs::ObservabilityAdapter`, moved from `sc-ai/src/observe.rs` and
     `sc-jev-py/src/logging.rs`, implementing `SessionLog` and mapping
     `EventError → LogRejected`. It keeps a-10's one-logger-per-root and flush
     rules.
   - `paths::{CliPathPolicy, PythonPathPolicy}`, each with
     `resolve_log_root` / `resolve_session_dir` / `jsonl_path`, encoding
     ADR-SAG-0021 **per root, unchanged**. The two policies stay distinct.
   - `compose::run_from(source: CheckSource, inputs: ComposeInputs) -> Result<RunResponse, JevError>`,
     which calls `discover_repo_root`, builds `RunConfig`, and calls
     `prepare`/`run`.
3. `sc-ai` and `sc-jev-py` delete their local adapters, path defaults, backend
   parsing (they use `BackendId::from_str`) and response shaping (they use
   `RunResponse::to_envelope_data`).

### D7 workspace-tests (REQ-SAG-0109)

1. `crates/workspace-tests` (`publish = false`, workspace member, dev-deps on
   all crates). It hosts `jev_cross_impl` (moved from the `[[test]]` in
   `sc-jev-py/Cargo.toml`, which is deleted) and a new
   `e2e_treesitter_producer`. That test builds the `sc-treesitter` binary
   (`env!("CARGO_BIN_EXE_…")` via an artifact dependency, or
   `escargot`-style `cargo build -p sc-treesitter --features cli`) and runs a
   `check.toml` with `kind = "command"`, `path = "src/sample.py"`, and argv
   containing `{path}`. It asserts the subjects digest equals the committed
   `branches` fixture, and that a bad language produces
   `LANGUAGE_UNAVAILABLE` re-raised through the engine (r-2 D11).

### D12 clap (REQ-SAG-0712, ADR-SAG-0010 amendment)

1. `clap` (derive) is optional, behind the `cli` feature that r-4 creates, in
   both `sc-ai` and `sc-treesitter`. The `sc-treesitter` library and the
   `sc-ai` library build without it (`--no-default-features`).
2. `sc-ai`: `#[derive(Parser)] struct Cli { #[command(subcommand)] cmd: Cmd }`
   with `Jev(JevCmd)`; `--rev` `conflicts_with = "request_json"`; values
   starting with `-` accepted via `allow_hyphen_values` on value flags. Parse
   uses `try_parse_from`: `DisplayHelp`/`DisplayVersion` print and exit 0;
   every other error goes to stderr as `ScAgentErrorResponse` with the same
   code current tests expect, exit 2. The a-10 edge cases are ported to
   `crates/sc-ai/tests/argv.rs`. `tests/cli.rs` is not edited (it belongs to r-2b).
3. `sc-treesitter`: same pattern, with `--version` and the existing flag set.
   Delete the hand-rolled parsers and their three flag tables in each binary.

### Manifests

Root `Cargo.toml` members (`sc-agent-compose`, `workspace-tests`); consumer
input crate `sc-agent-compose` with `publish_order` after `sc-jev-check`;
boundary manifests for compose (forbidden: sc-jev, sc-jev-backends,
sc-agent-http, pyo3) and the updated sc-ai/sc-jev-py/sc-treesitter edges.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0109 | `cargo test -p workspace-tests` runs `jev_cross_impl` and `e2e_treesitter_producer`; `sc-jev-py/Cargo.toml` has no `[[test]]` |
| req:REQ-SAG-0712 | No hand-rolled argv tables in either binary; `tests/argv.rs` edge cases green; `--no-default-features` lib builds pull no clap |
| adr:ADR-SAG-0026 | Status Active; `sc-ai`/`sc-jev-py` contain no adapter or path-default code (grep test) |
| adr:ADR-SAG-0016 | Compose bullet Active |
| adr:ADR-SAG-0010 | Amendment Active |
| adr:ADR-SAG-0021 | `test_cli_path_policy_precedence`, `test_python_path_policy_precedence` in compose reproduce every existing precedence case |
| boundary:agent-compose | Manifest enforced |
| boundary:sc-ai-cli | Regression: REQ-SAG-0700–0711 tests green |

## Required validation

```bash
just lint
just test
cargo test -p workspace-tests -- --nocapture
cargo test -p workspace-tests -- e2e_treesitter_producer --nocapture
cargo test -p sc-agent-compose -- --nocapture
cargo test -p sc-ai --test argv
cargo test -p sc-treesitter --test argv
cargo build -p sc-ai --lib --no-default-features
cargo build -p sc-treesitter --lib --no-default-features
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
uv run --directory tests/fixtures/jev-python pytest -q
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-agent-http -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-agent-compose -p sc-treesitter -p sc-ai
```

## This sprint does not close

MCP server (Phase B); removal of the v2 shim (`agt-obs-v2-shim-removal`).
