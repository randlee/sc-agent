# Decision: Crate layering and error values

**Status:** Approved  
**Version:** 0.2.0  
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

Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` (over `sc-decision-systemone`, whose Jev encode and decode it reuses) and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only. Above the runner, the `decidr` crate is the top layer for decision models: its library target is the one place a backend is chosen and wired into `sc-decision-runner`, and its binary is the `decidr` CLI. `sc-agent-py` is a thin PyO3 layer over the `decidr` library. CLI agents and SDK agents get their own crates later. `sc-treesitter` stands alone. Every call returns a Result whose error has an id and a message and keeps its cause.

**Key Principles:**

- A crate depends only on lower layers.
- `sc-decision-runner` launches decision models only.
- `sc-agent-log` takes generic rows, so it needs no decision types; the runner builds decision rows.
- Backends are chosen and wired in one place, the `decidr` library; `sc-agent-py` adds no wiring of its own.

### Rationale

Separate crates keep each launcher small and let a non-decision program use HTTP, providers and logging without decision types. Errors that keep their id, message and cause carry each failure's meaning upward.

### Consequences

**Positive:**

- A new launcher reuses the lower layers without touching decision code.

**Negative:**

- More crates to version and publish.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "always return descriminated unions w/ error info"

### Alternatives Considered

#### Alternative 1: One general engine crate for every launcher

**Description:**

Put decision, CLI-agent and SDK-agent launching in one crate.

**Pros:**

- Fewer crates.

**Cons:**

- Every launcher pulls in every other launcher's code.

**Why Rejected:**

The owner wants decision launching kept separate from other launching.

#### Alternative 2: One shared error type

**Description:**

Every crate returns the same error enum.

**Pros:**

- One list of error ids.

**Cons:**

- Crates that never touch a decision model carry decision errors.

**Why Rejected:**

A crate below the decision layer must not carry decision errors (layering).