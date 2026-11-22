# Decision: Crate layering and error values

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0004: Crate layering and error values

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0002
- REQ-SAG-0009
- REQ-SAG-0301

### Context

sc-agent will launch decision models, LLMs, CLI agents and SDK agents. Each needs shared types, configuration, secrets, HTTP and logging, but not each other's code.

**Problem Statement:**

Crates must be layered so each launcher depends only on what it uses, and errors must keep every lower-layer id and message.

### Decision

There are five crates. `sc-agent` is the bottom layer: the provider tables, `send` and `send_batch`, resends, keys, configuration, errors, logging paths and session rows. `sc-treesitter` (tree-sitter extraction) is over `sc-agent`. On top: `decidr` (the decision CLI), `sitr` (the tree-sitter CLI) and `sc-agent-py` (the one wheel, holding the decidr and sitr APIs). CLI agents and SDK agents get their own crates later. Every call returns a Result; the error is the one core error enum, with an id, a message and an action, and a layer that changes an error keeps the lower error's id and message (REQ-SAG-0002).

**Key Principles:**

- A crate depends only on lower layers.
- The provider is chosen in one place, `sc-agent`.

### Rationale

One core crate keeps the code small: ureq, keyring, dotenvy and sc-observability are called directly, with no traits or wrappers between them. Errors that keep their id and message carry each failure's meaning upward.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Consequences

**Positive:**

- A new launcher reuses `sc-agent` without touching the other launchers.

**Negative:**

- A program that wants only HTTP or logging still links the whole core.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "always return descriminated unions w/ error info"

### Alternatives Considered

#### Alternative 1: One crate per layer

**Description:**

Split types, configuration, secrets, logging, HTTP, schema, each backend and the runner into their own crates.

**Pros:**

- A program can depend on one layer alone.

**Cons:**

- Seventeen crates, with traits and wrappers between them, for what about 300 lines of Python did.

**Why Rejected:**

The reduction plan replaces it with one core crate written fresh to the spec.

#### Alternative 2: One error type per crate

**Description:**

Each crate returns its own error enum, wrapping the errors of the crates below it.

**Pros:**

- A crate carries only its own error ids.

**Cons:**

- Wrapper variants and copy-up code in every crate.

**Why Rejected:**

The reduction plan has one error enum for the core, logged and returned the same way from the CLI and the wheel.