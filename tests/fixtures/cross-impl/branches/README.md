# Cross-impl branches fixture (Phase A gate)

**Producer (a-3b):** `sc-treesitter` emits `subjects.json` from hook source + `queries/python-branches.scm` (also shipped under `sc-treesitter` assets).

**Consumers (a-3 / Python):** read the **same committed** `subjects.json`; build identical check request bytes + sha256.

Artifacts:

- Hook source (e.g. `hook.py` or path documented in fixture)
- `queries/python-branches.scm` (reference query; canonical copy in `sc-treesitter` assets)
- **`subjects.json`** — committed output of `sc-treesitter extract` (not Python `python-branches` extractor)
- `expected.request.sha256` / `expected.request.body.bin` — full check payload

Rust `sc-jev-check` and Python `jev-check` tests must match request digest.  
**No** atm-core PR #2308 dependency.
