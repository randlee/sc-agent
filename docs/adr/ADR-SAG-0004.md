# Decision: Crate layering and error values

**Status:** Approved  
**Version:** 0.4.0  
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

There are five crates. `sc-agent` is the bottom layer: the provider tables, `send` and `send_batch`, resends, keys, configuration, errors, logging paths and session rows. `sc-treesitter` (tree-sitter extraction) is over `sc-agent`. On top: `decidr` (the decision CLI), `sitr` (the tree-sitter CLI) and `sc-agent-py` (the one wheel, holding the decidr and sitr APIs). CLI agents and SDK agents get their own crates later. Errors are discriminated unions: every call returns `Result<T, E>`, and `E` carries the error info: at least an id and a message. A layer that changes an error keeps the lower error's id and message (REQ-SAG-0002). An error may carry more fields. Whether a crate has its own error type or shares one is not specified.

**Key Principles:**

- A crate depends only on lower layers.
- The provider is chosen in one place, `sc-agent`.

### Rationale

One core crate keeps the code small: ureq, keyring, dotenvy and sc-observability are called directly, with no traits or wrappers between them.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling (2026-11-21), Rand Lee: "Errors are discriminated unions Result<t,e> I don't care about per crate"; "Extra fields are fine, make sure adr doesn't limit too much. I don't want findings for returning more."

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
