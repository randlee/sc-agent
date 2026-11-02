# Cross-impl branches fixture (Phase A gate)

One hook source file, `queries/python-branches.scm`, and committed reference
artifacts from the Python `jev-check` package (after atm-core PR #2308 slice change):

- `expected.subjects.json` — subjects from Python reference
- `expected.request.sha256` / `expected.request.body.bin` — full check payload

Rust `sc-jev-check` and Python `sc-jev-py` tests must produce byte-identical
subjects and request digest for this input.
