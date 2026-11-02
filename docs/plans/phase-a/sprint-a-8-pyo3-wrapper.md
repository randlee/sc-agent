---
id: a-8
phase: a
status: planned
closure_type: integration
target_boundary: jev
track: python
wave: 3
branch: sprint/a-8-pyo3-wrapper
worktree: sprint/a-8-pyo3-wrapper
recommended_agent: tai
recommended_model: gpt-5.6-terra-medium
must_follow: [a-6]
parallel_safe: [a-7]
requirements:
  - REQ-SAG-0800
  - REQ-SAG-0801
  - REQ-SAG-0802
  - REQ-SAG-0803
adrs:
  - ADR-SAG-0011
  - ADR-SAG-0017
owned_paths:
  - "crates/sc-jev-py/**"
  - "tests/fixtures/jev-python/**"
  - "release/publish-artifacts.toml"
  - ".github/workflows/*publish*"
---

# Sprint a-8 — sc-jev-py (PyO3 + PyPI)

## Outcome

**`sc-jev-py`** is the supported Python integration: PyO3 bindings over
`sc-jev`, built with **maturin**, published as a **PyPI wheel** on the first
release alongside **crates.io** Rust crates and the **`sc-ai`** CLI.

## Deliverables

1. **REQ-SAG-0800** — PyO3 module: `build_request`, `run_check`, session types; exceptions carry `JevError` codes.
2. **REQ-SAG-0801** — Cross-impl test harness under `tests/fixtures/jev-python`.
3. **REQ-SAG-0802** — `pyproject.toml` + maturin config; `pip install` / `uv add` works from wheel (local `maturin build` in CI).
4. **REQ-SAG-0803** — Release wiring: PyPI + crates.io publish documented in `release/publish-artifacts.toml` and CI (or release checklist); version aligned with Rust workspace.
5. **ADR-SAG-0011** — Python must not reimplement Jev HTTP or payload canonicalization.
6. **ADR-SAG-0017** — First release = three surfaces (Rust / Python wheel / `sc-ai`).

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

## Acceptance criteria

```bash
just lint
cargo test -p sc-jev-py -- --nocapture
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-jev-py-wheels
uv run --directory tests/fixtures/jev-python pytest -q
python3 -m pip install --force-reinstall /tmp/sc-jev-py-wheels/sc_jev_py-*.whl
python3 -c "import sc_jev_py; assert hasattr(sc_jev_py, 'run_check')"
```

Expected: exit 0; wheel imports on supported platform. PyPI upload may be
**dry-run** in CI until credentials exist; release PR must list publish commands.

## Parity checklist

Library API for Python callers; cross-impl bytes test where fixture applies.

## This sprint does not close

Migrating atm-bd / orchestration / plan-anchor call sites in other repos (use
published wheel after release).
