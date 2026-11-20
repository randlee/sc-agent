# Decision: sc-treesitter-py and sitr are separate crates over sc-treesitter

**Status:** Approved
**Version:** 0.2.0
**Created:** 2026-11-09
**Last Updated:** 2026-11-19
**Owner:** Rand Lee
**Decision Date:** 2026-11-19

---

## ADR-SAG-0005: sc-treesitter-py and sitr are separate crates over sc-treesitter

**Status:** Approved

### Related Documents

**Requirements:**

- REQ-SAG-0301
- REQ-SAG-0303

**Architecture Decisions:**

- ADR-SAG-0004

### Context

sc-treesitter is a stand-alone library used by two query surfaces: the PyO3 bindings and the `sitr` CLI. Since 2026-11-12, the repository has built the CLI as its own `crates/sitr` crate.

**Problem Statement:**

The Python bindings need PyO3, and the CLI needs its own command-line and logging dependencies. A Rust caller of the library needs none of those dependencies or CLI targets.

### Decision

`sc-treesitter-py` is a separate PyO3 crate over `sc-treesitter`. `sc-treesitter` has a `python` feature that enables its PyO3 types. The `sitr` CLI is built from its own crate, `crates/sitr`, and depends on `sc-treesitter` for query functionality. `sc-treesitter` has no `sitr` binary target or `cli` feature. Its default features carry no PyO3, clap or sc-observability.

**Key Principles:**

- A default build of `sc-treesitter` pulls in no PyO3, clap or sc-observability.
- `sitr` is built from the separate `sitr` crate.
- The Python extension module is built from `sc-treesitter-py`, not from `sc-treesitter`.

### Rationale

The separate CLI crate keeps the library free of the CLI's clap and sc-observability dependencies and binary target. The separate PyO3 crate keeps the extension-module build out of the library.

### Consequences

**Positive:**

- A default Rust build of `sc-treesitter` compiles no Python or CLI dependencies.

**Negative:**

- Separate crates and a Python feature to version and test.

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

#### Alternative 2: Put sitr in sc-treesitter behind a cli feature

**Description:**

Put the `sitr` binary in `sc-treesitter` and gate it behind `required-features = ["cli"]`.

**Pros:**

- One fewer crate to version.

**Cons:**

- The library manifest contains a CLI binary target and carries its CLI feature and dependency wiring.

**Why Rejected:**

Keeping `sitr` in `crates/sitr` keeps the library free of the CLI's clap and sc-observability dependencies and binary target.
