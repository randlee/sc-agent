# Decision: sc-treesitter-py is a separate PyO3 crate; sitr lives in sc-treesitter behind a cli feature

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0005: sc-treesitter-py is a separate PyO3 crate; sitr lives in sc-treesitter behind a cli feature

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0301
- REQ-SAG-0303

**Architecture Decisions:**

- ADR-SAG-0004

### Context

sc-treesitter is a stand-alone library with two query surfaces: PyO3 bindings for Python and the `sitr` CLI.

**Problem Statement:**

The Python bindings and the CLI need PyO3, clap and sc-observability, but a Rust caller of the library needs none of them.

### Decision

`sc-treesitter-py` is a separate PyO3 crate over `sc-treesitter`. `sc-treesitter` has a `python` feature that enables its PyO3 types and a non-default `cli` feature. The `sitr` binary lives in `sc-treesitter` behind `cli` (`required-features = ["cli"]`). Default features carry no clap and no sc-observability.

**Key Principles:**

- A default build of `sc-treesitter` pulls in no PyO3, clap or sc-observability.
- `sitr` builds only when the `cli` feature is on.
- The Python extension module is built from `sc-treesitter-py`, not from `sc-treesitter`.

### Rationale

Feature gates keep the library's default dependency set small while the PyO3 types stay next to the code they wrap; a separate PyO3 crate keeps the extension-module build out of the library.

### Consequences

**Positive:**

- A Rust caller of `sc-treesitter` compiles no Python or CLI dependencies.

**Negative:**

- Two crates and two features to version and test.

### Alternatives Considered

#### Alternative 1: PyO3 bindings and CLI inside sc-treesitter with no feature gates

**Description:**

Build the extension module and `sitr` from `sc-treesitter` with all dependencies always on.

**Pros:**

- One crate.

**Cons:**

- Every caller compiles PyO3, clap and sc-observability.

**Why Rejected:**

Default features must carry no clap or sc-observability.

#### Alternative 2: Separate sitr crate

**Description:**

Put the `sitr` binary in its own crate.

**Pros:**

- The library manifest has no binary target.

**Cons:**

- One more crate for a single small binary.

**Why Rejected:**

A `cli` feature with `required-features` gives the same isolation without another crate.