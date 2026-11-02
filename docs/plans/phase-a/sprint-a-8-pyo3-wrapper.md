---
id: a-8
phase: a
status: planned
closure_type: integration
target_boundary: jev-py
track: python
wave: 3
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
  - ADR-SAG-0011
  - ADR-SAG-0016
owned_paths:
  - "crates/sc-jev-py/**"
  - "boundaries/jev-py/**"
  - "tests/fixtures/jev-python/**"
---

# Sprint a-8 — sc-jev-py (PyO3 + PyPI)

## Outcome

**`sc-jev-py`** is the supported Python integration: PyO3 bindings over
**`sc-jev-check`**, built with **maturin**, PyPI-ready wheel (publish in **a-9/a-10**).

## Deliverables

1. **REQ-SAG-0800** — PyO3 module: `build_request`, `run_check`, session types; exceptions carry `JevError` codes.
2. **REQ-SAG-0801** — Cross-impl test harness under `tests/fixtures/jev-python`.
3. **REQ-SAG-0802** — `pyproject.toml` + maturin config; `pip install` / `uv add` works from wheel (local `maturin build` in CI).
4. **ADR-SAG-0011** — Python must not reimplement Jev HTTP or payload canonicalization.
5. **ADR-SAG-0016** — **Required** `sc-observability = "1.5.0"` pin; adapter implements `SharedSessionLog` for in-process Python (same JSONL shape as CLI when log dir set).
6. Integration test: one `run_check` with temp log dir appends **one** JSONL line per judge send; line contains **no** API key material.

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
| req:REQ-SAG-0800 | `run_check` + session `ask` through wheel |
| req:REQ-SAG-0801 | Cross-impl pytest matches a-1 payload digest **and** branches fixture |
| req:REQ-SAG-0610 | JSONL line per judge send when log dir set |
| req:REQ-SAG-0611 | JSONL contains no API key material |
| boundary:jev-py | `cargo tree -p sc-jev-py --depth 1` lists `sc-jev-check`, not `sc-jev` |

## Required validation

```bash
just lint
cargo test -p sc-jev-py -- --nocapture
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-jev-py-wheels
uv run --directory tests/fixtures/jev-python pytest -q
python3 -m pip install --force-reinstall /tmp/sc-jev-py-wheels/sc_jev_py-*.whl
uv run --directory tests/fixtures/jev-python pytest -q tests/test_run_check_and_ask.py
uv run --directory tests/fixtures/jev-python pytest -q tests/test_cross_impl_bytes.py
uv run --directory tests/fixtures/jev-python pytest -q tests/test_cross_impl_branches.py
! cargo tree -p sc-jev-py --depth 1 | rg -q ' sc-jev '
```

Expected: exit 0; pytest exercises `run_check`, session `ask`, cross-impl digest,
and maps `JevError` to Python exceptions (no HTTP reimplementation).

## Parity checklist

See **plan.md** parity rows for sprint **a-8**.

## This sprint does not close

Migrating atm-bd / orchestration / plan-anchor call sites in other repos (use
published wheel after release).
