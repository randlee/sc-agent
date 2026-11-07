---
id: a-8
phase: a
status: complete
closure_type: integration
target_boundary: jev-py
track: python
wave: 4
branch: sprint/a-8-pyo3-wrapper
worktree: sprint/a-8-pyo3-wrapper
recommended_agent: tai
recommended_model: gpt-5.6-terra-medium
must_follow: [a-6]
parallel_safe: [a-7]
requirements:
  - REQ-SAG-0102
  - REQ-SAG-0610
  - REQ-SAG-0611
  - REQ-SAG-0800
  - REQ-SAG-0801
  - REQ-SAG-0802
  - NFR-SAG-0002
  - NFR-SAG-0003
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0003
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0011
  - ADR-SAG-0012
  - ADR-SAG-0013
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0019
  - ADR-SAG-0020
  - ADR-SAG-0021
  - ADR-SAG-0022
owned_paths:
  - "crates/sc-jev-py/**"
  - "crates/sc-jev-check/src/run/**"
  - "boundaries/sc-jev-py/**"
  - "tests/fixtures/jev-python/**"
  - "docs/architecture.md"
  - "docs/sc-jev-check/architecture.md"
  - "docs/sc-jev-py/architecture.md"
  - "docs/plans/phase-a/plan.md"
---

# Sprint a-8 — sc-jev-py (PyO3 + PyPI)

## Outcome

**`sc-jev-py`** is the supported Python integration: PyO3 bindings over
**`sc-jev-check`**, built with **maturin**, PyPI-ready wheel (publish in **a-9**).

## Deliverables

1. **REQ-SAG-0800** — PyO3 module: `build_request`, `run_check`, session types; exceptions carry `JevError` codes. `build_request` calls `sc_jev_check::run::build_request` and does not run extractors or call `PayloadBuilder` (ADR-SAG-0005). Live runs leave `resolve_key` to `sc-jev-check` (ADR-SAG-0003, ADR-SAG-0020). This crate does not call `resolve_key`.
2. **REQ-SAG-0801** — Cross-impl test harness under `tests/fixtures/jev-python`.
3. **REQ-SAG-0802** — `pyproject.toml` + maturin config; `pip install` / `uv add` works from a locally built wheel. Proof is the `maturin build` and pytest commands in Required validation. Wheel CI is the `python` job of `.github/workflows/sc-agent-ci.yml` (step `maturin develop + pytest (just test-python)`): it builds the `extension-module,test-util` extension into a venv and runs `tests/fixtures/jev-python` pytest on ubuntu-24.04, macos-15 and windows-2022. Sprint a-9 names that step as its wheel+pytest CI deliverable; it landed in the Phase A phase-end hardening (PR #18).
4. **ADR-SAG-0011** — Python must not reimplement Jev HTTP or payload canonicalization.
5. **ADR-SAG-0016** — **Required** `sc-observability = "1.5.0"` pin; adapter implements `SharedSessionLog` for in-process Python (same JSONL shape as CLI when log dir set).
6. Integration test: one `run_check` with temp log dir appends **one** JSONL line per judge send; line contains **no** API key material.
7. **NFR-SAG-0003** / **ADR-SAG-0021** — `resolve_log_root` and `resolve_session_dir` follow the Python order in ADR-SAG-0021. Log root is `log_dir`, then `SC_AGENT_LOG_DIR`, then `SC_LOG_ROOT`, then `~/.sc-agent/log` (`%USERPROFILE%\.sc-agent\log` when `HOME` is unset; the process temp directory plus `sc-agent/log` when both are unset). Session dir is `session_dir`, then `SC_AGENT_SESSION_DIR`, then `{log_root}/sessions`. ADR-SAG-0013 still governs the v2 `LoggerBuilder` file sink and forbids a bespoke JSONL writer. CLI precedence is the other half of ADR-SAG-0021 (`--log-dir`, then `SC_LOG_ROOT`, then `<repo-root>/.sc-ai`; `--sessions-dir`, then `<repo-root>/.sc-ai/sessions`) and stays implemented in sprint a-7.
8. **ADR-SAG-0004** — `run_check` uses the facade path that calls `validate_for_send` before any judge send. This crate does not split an oversized check.
9. **ADR-SAG-0022** — Amends ADR-SAG-0009. Production `run_check` does not take `scripted_answers` and this crate does not implement `JudgeBackend`. `sc-jev-check` feature `test-util` exposes `register_scripted_backends` for tests. `sc-jev-py` feature `test-util` only forwards that call, through the separate test entry `sc_jev_py._testing.run_check_scripted`; `run_check` never takes `scripted_answers` (ADR-SAG-0029). Default features and the publish build leave both off. Required validation below enables `test-util` so pytest can exercise session `ask` without a network send.

## Explicit code samples

```python
import sc_jev_py as jev

result = jev.run_check("check.toml", backends=["jev"], dry_run=False)
```

```toml
# pyproject.toml (maturin)
[project]
name = "sc-jev-py"
requires-python = ">=3.11"
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0102 | Python cross-impl payload digest matches a-1 fixture |
| req:REQ-SAG-0800 | `run_check` + session `ask` through wheel |
| req:REQ-SAG-0801 | Cross-impl pytest matches a-1 payload digest **and** branches fixture |
| req:REQ-SAG-0802 | `pip install` wheel + pytest re-run succeeds; CI proof is `sc-agent-ci` job `python` (`just test-python`) |
| req:REQ-SAG-0610 | JSONL line per judge send when log dir set |
| req:REQ-SAG-0611 | JSONL contains no API key material |
| evidence:NFR-SAG-0002 | Boundary evidence only; the `req:` root is owned by **a-7**. No secrets in Python-visible output or JSONL. Tests: `crates/sc-jev-py/src/logging.rs::adapter_writes_one_line_without_secret_fields` (no `api_key`/secret field in the JSONL line); `crates/sc-jev-py/src/run.rs::tests::live_run_appends_one_jsonl_line_per_send_without_the_prompt` (sentinel question text and `api_key` absent from both JSONL lines); `tests/fixtures/jev-python/tests/test_run_check_and_ask.py::test_run_check_and_ask_append_jsonl_without_secrets` and `::test_jev_ask_and_missing_check_map_error_codes` (sentinel key `sk-test-sentinel-not-a-real-key` absent from the result payload, JSONL lines, `ask` result and exception text). Recorded run: `sc-agent-ci` [42345904358](https://github.com/randlee/sc-agent/actions/runs/42345904358), jobs `test` and `python` on three OSes |
| evidence:NFR-SAG-0003 | Boundary evidence only; the `req:` root is owned by **a-7**. `resolve_log_root` and `resolve_session_dir` match the Python half of ADR-SAG-0021 (`test_log_root_resolution`) |
| boundary:jev-py | `cargo tree -p sc-jev-py --depth 1` lists `sc-jev-check`, not `sc-jev` |

## Required validation

```bash
just lint
cargo test -p sc-jev-py -- --nocapture
# test-util is off by default. This proof wheel forwards scripted answers
# to sc-jev-check. The publish build (a-9) does not pass --features test-util.
maturin build -m crates/sc-jev-py/Cargo.toml --features extension-module,test-util -o /tmp/sc-jev-py-wheels
uv run --directory tests/fixtures/jev-python pytest -q
python3 -m pip install --force-reinstall /tmp/sc-jev-py-wheels/sc_jev_py-*.whl
uv run --directory tests/fixtures/jev-python pytest -q tests/test_run_check_and_ask.py
uv run --directory tests/fixtures/jev-python pytest -q tests/test_cross_impl_bytes.py
uv run --directory tests/fixtures/jev-python pytest -q tests/test_cross_impl_branches.py
! cargo tree -p sc-jev-py --depth 1 | rg -q ' sc-jev '
```

Expected: exit 0; pytest exercises `run_check`, session `ask`, cross-impl digest,
and maps `JevError` to Python exceptions (no HTTP reimplementation). The pytest
extension is the `test-util` build (`tests/fixtures/jev-python` passes that
Cargo feature to maturin) and calls `sc_jev_py._testing.run_check_scripted`
for scripted answers. `run_check` never takes `scripted_answers`; default
features omit the `_testing` submodule (ADR-SAG-0029).

## Parity checklist

See **plan.md** parity rows for sprint **a-8**.

## This sprint does not close

Migrating atm-bd / orchestration / plan-anchor call sites in other repos (use
published wheel after release).

Pull-request CI that builds the wheel and runs jev-python pytest is not this
sprint's deliverable. It is the `python` job of `.github/workflows/sc-agent-ci.yml`
(`just test-python`), named in sprint **a-9** and added at the Phase A
phase-end hardening.
