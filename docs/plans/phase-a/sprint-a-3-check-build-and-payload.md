---
id: a-3
phase: a
status: planned
closure_type: boundary
target_boundary: jev
track: build
wave: 1
branch: sprint/a-3-check-build-and-payload
worktree: sprint/a-3-check-build-and-payload
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [a-1]
parallel_safe: [a-2]
requirements:
  - REQ-SAG-0300
  - REQ-SAG-0301
  - REQ-SAG-0302
  - REQ-SAG-0303
  - REQ-SAG-0304
adrs:
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0015
owned_paths:
  - "crates/sc-jev/**"
  - "crates/sc-jev/assets/jev.toml"
  - "boundaries/jev/**"
  - "tests/fixtures/jev/git-repo/**"
---

# Sprint a-3 — check build, extractors, payload

## Outcome

`sc-jev` loads `check.toml`, runs git extractors at repo root, expands
question sets, builds canonical payload bytes, validates `INPUT_*` without
sending. Vendor pin defaults live in packaged `jev.toml` (tighten **#12**).

## Deliverables

1. **REQ-SAG-0300** — Extractors: `file`, `table-column`, **`delimited-regions`**.
2. **REQ-SAG-0304** — Multiple distinct region ids per file; optional `regions`
   list to extract only those ids (others ignored). Markers per ADR-SAG-0015.
   **No** Python AST / `python-branches` in this repo.
3. **REQ-SAG-0301** — `CONFIG_INVALID`, `INPUT_MISSING`, `EXTRACTION_EMPTY`, `INPUT_INVALID`, `INPUT_TOO_LARGE` before send.
4. **REQ-SAG-0302** — Repo-root resolution via `git rev-parse --show-toplevel` (tighten **#8**).
5. **REQ-SAG-0303** — Question-set schema `plan-anchoring/questions/1` expansion.
6. **ADR-SAG-0004** — One check = one request payload bundle.
7. **ADR-SAG-0005** — Max request bytes 96000 from config.
8. **ADR-SAG-0015** — Delimiter contract; language-specific extractors live with consumers.

## Explicit code samples

```toml
[[extractors]]
id = "plan-arms"
type = "delimited-regions"
path = "src/policy.py"
rev = "HEAD"
regions = ["allow"]   # omit to include every closed region in the file
```

```python
# sc-jev:region:allow
if user.is_admin():
    grant()
# sc-jev:end:allow

# sc-jev:region:legacy   # ignored when regions = ["allow"]
old_path()
# sc-jev:end:legacy
```

## Acceptance criteria

```bash
just lint
cargo test -p sc-jev build -- --nocapture
cargo test -p sc-jev extractors -- --nocapture
cargo test -p sc-jev delimited_regions -- --nocapture
cargo test -p sc-jev payload -- --nocapture
```

Expected: exit 0; fixtures include `.py` and `.rs` with markers. README row
`test_python_branches_lists_arms_and_refusals` is **out of Phase A scope**
(consumer-owned); do not block phase close on it.

## Parity checklist

Config, `file`, `table-column`, delimited-regions, request build, payload bytes,
oversized input — not Python AST branches.

## This sprint does not close

HTTP backends, sessions, CLI, in-crate language parsers.
