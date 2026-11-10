# Decision: Crate layering and per-crate error unions

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0004: Crate layering and per-crate error unions

**Status:** Approved  

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

Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` (over `sc-decision-systemone`, whose Jev encode and decode it reuses) and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only. Above the runner, the `decide` crate is the top layer for decision models: its library target is the one place a backend is chosen and wired into `sc-decision-runner`, and its binary is the `decide` CLI. `sc-agent-py` is a thin PyO3 layer over the `decide` library. CLI agents and SDK agents get their own crates later. `sc-treesitter` stands alone. `sc-agent-types` defines the `ErrorId` trait and `ErrorDetail` (id, message, cause) that every sc-agent crate's error union uses; `sc-treesitter` defines its own. Each crate defines its own error union, except a crate whose public functions all return the error type fixed by a trait it implements (such as `sc-decision-systemone`, which returns `DecisionError` through `Backend`); a union wraps the error of the layer below it; an error from a crate that is not below (such as an LLM error inside a decision error) is carried as its id, message and cause chain.

**Key Principles:**

- A crate depends only on lower layers.
- `sc-decision-runner` launches decision models only.
- There is no shared error type for all crates; `HttpError` is wrapped by `LlmError` and by `DecisionError`, and an `LlmError` reaches `DecisionError` as its id, message and cause chain.
- `sc-agent-log` takes generic rows, so it needs no decision types; the runner builds decision rows.
- Backends are chosen and wired in one place, the `decide` library; `sc-agent-py` adds no wiring of its own.
- Exemption: a crate whose public functions all return the error type fixed by a trait it implements needs no error union of its own; `sc-decision-systemone` returns `DecisionError` through `Backend` and defines no union.

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