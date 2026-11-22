# Decision: sitr's Python API is built into the sc-decidr wheel; sitr is its own crate

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-22  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0005: sitr's Python API is built into the sc-decidr wheel; sitr is its own crate

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0301
- REQ-SAG-0303

**Architecture Decisions:**

- ADR-SAG-0004

### Context

sc-treesitter is the tree-sitter library with two query surfaces: the Python API and the `sitr` CLI.

**Problem Statement:**

The Python bindings and the CLI need PyO3 and clap, but a Rust caller of the library needs neither.

### Decision

`sc-agent-py`, the crate that builds the sc-decidr wheel, depends on `sc-treesitter` and exposes the sitr Python API as `sc_agent.sitr`; there is no `sc-treesitter-py` crate (ADR-SAG-0021). There is no separate sitr wheel in phase h; a standalone sitr wheel may return after the MVP, and a process uses either it or the sc-decidr wheel, never both. The `sitr` binary is its own crate, `sitr`, over `sc-treesitter`. `sc-treesitter` has no PyO3 and no clap.

**Key Principles:**

- `sc-treesitter` pulls in no PyO3 or clap.
- The one Python extension module is built from `sc-agent-py`, not from `sc-treesitter`.

### Rationale

Keeping PyO3 in `sc-agent-py` and clap in `sitr` keeps the library's dependency set small; building the sitr API into the sc-decidr wheel gives a Python process one copy of sc-observability and one Logger for decidr and sitr calls (ADR-SAG-0021).

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Consequences

**Positive:**

- A Rust caller of `sc-treesitter` compiles no Python or CLI dependencies.

**Negative:**

- Installing the wheel always installs both the decidr and sitr APIs.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "I was planning to bring the sitr wheel back once we get mvp working."

### Alternatives Considered

#### Alternative 1: PyO3 bindings and CLI inside sc-treesitter with no feature gates

**Description:**

Build the extension module and `sitr` from `sc-treesitter` with all dependencies always on.

**Pros:**

- One crate.

**Cons:**

- Every caller compiles PyO3, clap and sc-observability.

**Why Rejected:**

The library must carry no PyO3 or clap.

#### Alternative 2: sitr inside sc-treesitter behind a cli feature

**Description:**

Build the `sitr` binary from `sc-treesitter` behind a non-default `cli` feature.

**Pros:**

- One crate fewer.

**Cons:**

- Feature gates and `required-features` in the library manifest.

**Why Rejected:**

The reduction plan's layout lists `sc-treesitter` and `sitr` as two parts of one row, and the rewrite builds them as two crates; a feature gate would add manifest rules for no gain.
