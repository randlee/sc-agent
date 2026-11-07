# CI and local checks

Two consumer-owned workflows run on pull requests. Neither is a kit file;
`install.py` never writes them.

| Workflow | Triggers | Purpose |
| --- | --- | --- |
| `.github/workflows/sc-agent-ci.yml` | pull requests; pushes to `develop` and `integrate/**`; manual | Lint, tests, Python harness, feature matrix, rustdoc, coverage |
| `.github/workflows/sc-publish-dry-run.yml` | pull requests; manual | Release surface: kit drift check, preflight, `cargo publish --dry-run`, publish-surface wheel ([`sc-publish.md`](sc-publish.md)) |

## sc-agent CI jobs and local equivalents

Every job pins its actions to commit SHAs. Toolchain `1.94.1`, Python `3.11.9`,
maturin `1.16.0`, pytest `8.4.2` and just `1.58.0` are exact; maturin, pytest
and their transitive dependencies install from the hash-pinned
`tests/fixtures/jev-python/constraints.txt`. Every job sets
`core.autocrlf false` before checkout so Windows keeps LF in the committed
fixtures that tests compare byte for byte. Every step has a timeout. No step
retries; bounded cargo network-fetch retries only (`CARGO_NET_RETRY=2`).

| Job | Runners | Local equivalent |
| --- | --- | --- |
| `test` | ubuntu-24.04, macos-15, windows-2022 | `just setup` and `just lint` (ubuntu-24.04 only), then on every OS `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo test --workspace` |
| `python` | ubuntu-24.04, macos-15, windows-2022 | `just test-python` |
| `features` | ubuntu-24.04 | `cargo hack --each-feature check -p sc-treesitter` |
| `doc` | ubuntu-24.04 | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` |
| `coverage` | ubuntu-24.04 | `cargo llvm-cov --workspace --no-report`, then `cargo llvm-cov report` |

`just lint` runs the three `[[tool.sc-lint.lint]]` steps in `sc-lint.toml`:
`sc-lint lint sc-boundary` (see [`boundaries/README.md`](../../boundaries/README.md)
for what it enforces), `cargo fmt --all --check`, and
`cargo clippy --workspace --all-targets -- -D warnings`.

`just test` runs both `[[tool.sc-lint.test]]` layers: `workspace`
(`cargo test --workspace`) and `jev-python` (`just test-python`). Run one layer
with `just test workspace` or `just test jev-python`. CI runs the two layers in
separate jobs.

### `just test-python`

The recipe:

1. recreates (`venv --clear`) a venv at `$JEV_VENV`, default
   `target/jev-python-venv` (CI uses `$RUNNER_TEMP/jev-python-venv`, outside
   the cached `target/`), with the `python3` on `PATH` (`python` on Windows;
   3.11 or newer). On Windows the venv path must not contain spaces;
2. installs `maturin==1.16.0`, `pytest==8.4.2` and their dependencies with
   `pip install --require-hashes -r tests/fixtures/jev-python/constraints.txt`.
   The lock is passed as `-r` because pip 24.0 (bundled with Python 3.11.9)
   ignores constraint-file hashes for top-level requirements. To change a
   version, regenerate the file as its header describes;
3. runs `maturin develop --features extension-module,test-util -m crates/sc-jev-py/Cargo.toml`
   with `VIRTUAL_ENV` set to that venv;
4. runs `pytest tests/fixtures/jev-python -q`.

This is the **test build**. `test-util` lets pytest register scripted judges so
session `ask` runs without a network send. The publish wheel leaves
`test-util` off (ADR-SAG-0022); `sc-publish-dry-run.yml` builds that wheel.

`uv run --directory tests/fixtures/jev-python pytest -q` is an equivalent path
for developers who use uv: the fixture's `pyproject.toml` passes the same
features to maturin.

### Known limits

- **`doc` is `continue-on-error`.** The workspace has 11 rustdoc warnings at
  `d4a0111`, so the step shows green while it fails. Removal condition: QA-005
  (rustdoc warnings fixed) lands from the phase-end code track, and that
  change removes the flag.
- **`just setup` / `just lint` run on ubuntu-24.04 only.** On windows-2022 the
  managed sc-lint bootstrap (`.sc-lint/bootstrap.py`, sc-lint 0.8.0) prints
  "verified sc-lint release activated" and then fails with
  `...\AppData\Local\sc-lint\bin\sc-lint.exe: [WinError 2] The system cannot
  find the file specified` (exit 127; run 42344867011). The bootstrap is a
  managed file and is not patched here. Boundary, fmt and clippy results do
  not depend on the host; clippy `--all-features` and `cargo test` still run
  on all three OSes. Upstream issue 1 below.
- **`features` runs `check`, not `test`.** The per-language sc-treesitter tests
  are not feature-gated yet, so `--no-default-features` test builds fail. The
  job switches to `cargo hack --each-feature test -p sc-treesitter` once they
  are gated.
- **Coverage is reported, not gated.** The job uploads `coverage/summary.txt`
  and `coverage/summary.json` as the `coverage-summary` artifact and writes the
  table to the job summary. Nothing is sent to a third-party service.

## Upstream issues to file (sc-lint)

These belong to the sc-lint kit, not sc-publish (sc-publish kit issues are in
[`sc-publish.md`](sc-publish.md#upstream-kit-issues-to-file)). Not filed yet.

1. **Windows bootstrap activation.** `bootstrap.py setup` on windows-2022
   reports the verified release as activated, then cannot find
   `%LOCALAPPDATA%\sc-lint\bin\sc-lint.exe` (`[WinError 2]`, exit 127). The
   extraction or the activated path is wrong on Windows. When fixed, drop the
   `if: matrix.os == 'ubuntu-24.04'` guards in the `test` job.

## Managed files

`sc-lint.toml` and `justfile` say "Managed by sc-lint". The
`global_dependency_ownership` records and the `jev-python` test layer in
`sc-lint.toml`, and the `install-sc-ai` / `test-python` recipes in `justfile`,
are hand-added. `sc-lint init --just` regeneration would drop them; re-apply
them after regenerating.

## Tools to install locally

```bash
cargo install cargo-hack --version 0.6.46 --locked
cargo install cargo-llvm-cov --version 0.9.1 --locked
rustup component add llvm-tools-preview
```

`just setup` installs the sc-lint release that `sc-lint.toml` requires and
verifies its checksum.
