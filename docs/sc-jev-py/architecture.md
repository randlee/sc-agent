# sc-jev-py — architecture

**Boundary:** `jev-py`  
**Depends on:** `sc-jev-check` (Rust, statically linked in extension)

## Role

Official **Python integration** for TypeSafe Jev / check flows. Distributed as a
**PyPI wheel** (maturin). Python applications `import sc_jev_py` and call Rust
in-process — no subprocess to `sc-ai` required.

atm-bd `packages/jev-check` may remain a thin policy/typing wrapper over this
extension; it is not the primary deliverable.

## Build and release

- **maturin** builds the extension against workspace **`sc-jev-check`**.
- Version **lockstep** with Rust crates on first release (single tag / workspace version).
- CI produces wheels (manylinux, macOS arm64 minimum); **PyPI** publish aligns
  with crates.io and **Homebrew / Scoop / WinGet** for `sc-ai` on one tag.

## Contract

PyO3 only — no duplicate Jev HTTP or canonicalization. `build_request` calls
`sc_jev_check::run::build_request` and does not run extractors (ADR-SAG-0005).
Exceptions expose the same error codes as `JevError`.

Log and session paths follow ADR-SAG-0021. Log root, first match:
`log_dir`, `SC_AGENT_LOG_DIR`, `SC_LOG_ROOT`, then `~/.sc-agent/log`. Session
directory, first match: `session_dir`, `SC_AGENT_SESSION_DIR`, then
`{log_root}/sessions`. The JSONL file is `{log_root}/logs/sc-jev-py.log.jsonl`
through the v2 `LoggerBuilder` file sink (ADR-SAG-0013).
