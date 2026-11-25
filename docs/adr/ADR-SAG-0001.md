# Decision: Every provider receives one request form

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0001: Every provider receives one request form

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0201
- REQ-SAG-0006

### Context

sc-agent sends decisions to Jev, to local runners that serve the Jev API, and to LLM backends such as Haiku and Luna.

**Problem Statement:**

Jev, local Jev-compatible runners, Haiku and Luna must answer the same decision so their answers can be compared, without shared crates knowing how any one backend adapts it.

### Decision

Every provider (Jev, local Jev-compatible runners, Haiku, Luna) receives the same Jev API request body. `sc-agent` holds one static table per provider, and each table names its request shape: SystemOne, Messages or chat/completions. Before a send, the request is adapted to the provider's request shape; the reply is decoded back into a Jev API response. Callers use two functions, `send` for one request and `send_batch` for many; the worker threads, holdoff, cancellation and session logging live behind them. There are no traits and no wrapper request type.

**Key Principles:**

- There is one request form: the Jev API request body.
- Adaptation knowledge lives only in the request-shape adaptation.
- `send_batch` returns each result as it is ready, tagged with its session ULID and input position; a failed request is returned in its result and the batch continues.

### Rationale

When every backend receives the same data, their answers are comparable by construction, and shared code stays free of provider-specific types.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling (2026-11-21), Rand Lee: "I do not require results in request order."

### Consequences

**Positive:**

- A new provider is added as a table row, with no change to shared types.

**Negative:**

- Each request shape must translate the full Jev request.

### Alternatives Considered

#### Alternative 1: Canonical payload beside the Jev request

**Description:**

Build a backend-neutral payload and derive each backend's request from it.

**Pros:**

- Backends share one translation step.

**Cons:**

- Two forms of one request must be kept equal.
- Shared types learn backend detail.

**Why Rejected:**

The owner's design has exactly one request form.