# Decision: Crate layering and per-crate error unions

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0004: Crate layering and per-crate error unions

**Status:** Draft  

### Related Documents

**Requirements:**

- REQ-SAG-0002
- REQ-SAG-0009
- REQ-SAG-0301

### Context

sc-agent will launch decision models, LLMs, CLI agents and SDK agents. Each needs shared types, configuration, secrets, HTTP and logging, but not each other's code.

**Problem Statement:**

Crates must be layered so each launcher depends only on what it uses, and errors must keep every lower-layer id and message without one shared error type for everything.

### Decision

Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only. CLI agents and SDK agents get their own crates later. `sc-treesitter` stands alone. Each crate defines its own error union, which wraps the error of the layer below it; an error from a crate that is not below (such as an LLM error inside a decision error) is carried as its id, message and cause chain.

**Key Principles:**

- A crate depends only on lower layers.
- `sc-decision-runner` launches decision models only.
- There is no shared error type for all crates; `HttpError` is wrapped by `LlmError` and by `DecisionError`, and an `LlmError` reaches `DecisionError` as its id, message and cause chain.
- `sc-agent-log` takes generic rows, so it needs no decision types; the runner builds decision rows.

### Rationale

Separate crates keep each launcher small and let a non-decision program use HTTP, providers and logging without decision types. Per-crate errors keep each failure's meaning where it happens while carrying the full chain upward.

### Consequences

**Positive:**

- A new launcher reuses the lower layers without touching decision code.

**Negative:**

- More crates to version and publish.

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

Each crate owns its errors and wraps the lower ones.