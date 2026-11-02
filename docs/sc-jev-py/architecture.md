# sc-jev-py — architecture

**Boundary:** `jev` (Python surface)  
**Depends on:** `sc-jev` (Rust, statically linked in extension)

## Role

Official **Python integration** for TypeSafe Jev / check flows. Distributed as a
**PyPI wheel** (maturin). Python applications `import sc_jev_py` and call Rust
in-process — no subprocess to `sc-ai` required.

atm-bd `packages/jev-check` may remain a thin policy/typing wrapper over this
extension; it is not the primary deliverable.

## Build and release

- **maturin** builds the extension against workspace `sc-jev`.
- Version **lockstep** with Rust crates on first release (single tag / workspace version).
- CI produces wheels (manylinux, macOS arm64 minimum); publish to **PyPI** with
  Rust **crates.io** publish in the same release train as **`sc-ai`**.

## Contract

PyO3 only — no duplicate Jev HTTP or canonicalization. Exceptions expose the
same error codes as `JevError`.
