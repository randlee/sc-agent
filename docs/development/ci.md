# CI and local checks

Two consumer-owned workflows run on pull requests. Neither is a kit file;
`install.py` never writes them.

| Workflow | Triggers | Purpose |
| --- | --- | --- |
| `.github/workflows/sc-agent-ci.yml` | pull requests; pushes to `develop` and `integrate/**`; manual | Lint, tests, Python harness, feature matrix, rustdoc, coverage |
| `.github/workflows/sc-publish-dry-run.yml` | pull requests; manual | Release surface: kit drift check, preflight, `cargo publish --dry-run`, publish-surface wheel ([`sc-publish.md`](sc-publish.md)) |

## sc-agent CI jobs and local equivalents

Every job pins its actions to commit SHAs. Toolchain `1.94.1`, Python `3.11`,
maturin `1.16.0`, pytest `8.4.2` and just `1.58.0` are exact. Every step has a
timeout and nothing retries.

| Job | Runners | Local equivalent |
| --- | --- | --- |
| `test` | ubuntu-24.04, macos-14, windows-2022 | `just setup`, `just lint`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace` |
| `python` | ubuntu-24.04, macos-14, windows-2022 | `just test-python` |
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

1. creates a venv at `target/jev-python-venv` with the `python3` on `PATH`
   (`python` on Windows; 3.11 or newer);
2. installs `maturin==1.16.0` and `pytest==8.4.2` into it (override with
   `MATURIN_VERSION` / `PYTEST_VERSION`);
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

- **`doc` is `continue-on-error`.** The workspace has rustdoc warnings at
  `d4a0111`. The step reports them without failing the run until the
  phase-end code track fixes them; then the flag is removed.
- **`features` runs `check`, not `test`.** The per-language sc-treesitter tests
  are not feature-gated yet, so `--no-default-features` test builds fail. The
  job switches to `cargo hack --each-feature test -p sc-treesitter` once they
  are gated.
- **Coverage is reported, not gated.** The job uploads `coverage/summary.txt`
  and `coverage/summary.json` as the `coverage-summary` artifact and writes the
  table to the job summary. Nothing is sent to a third-party service.

## Tools to install locally

```bash
cargo install cargo-hack --version 0.6.46 --locked
cargo install cargo-llvm-cov --version 0.9.1 --locked
rustup component add llvm-tools-preview
```

`just setup` installs the sc-lint release that `sc-lint.toml` requires and
verifies its checksum.
