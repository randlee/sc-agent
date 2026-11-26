# Decision: Every decision provider receives one request form

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0001: Every decision provider receives one request form

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

For decision requests, every decision provider receives the same Jev API request body. `sc-agent` holds one provider table and one model table; the model table is its built-in rows plus the rows configuration adds (REQ-SAG-0102), so a new model works without a release. Decision providers implement one decision trait, and LLM providers one LLM trait (JSON in, JSON out). Jev implements the decision trait over SystemOne. One type per wire format, OpenAI-compatible chat/completions and Anthropic-compatible Messages, implements both traits and is shared by every provider of that format. Before a send, the decision implementation adapts the request to its wire format; the reply is decoded back into a Jev API response. Callers open one `Run` per command (created, used for its calls, then disposed) and use its two methods, `send` for one request and `send_batch` for many; the worker threads, holdoff, cancellation and session logging live behind them.

**Key Principles:**

- There is one decision request form: the Jev API request body.
- `send_batch` returns each result as it is ready, tagged with its session ULID and input position; a failed request is returned in its result and the batch continues.

### Rationale

When every backend receives the same data, their answers are comparable by construction, and shared code stays free of provider-specific types.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling (2026-11-21), Rand Lee: "I do not require results in request order."

### Consequences

**Positive:**

- A new provider is added as a table row, with no change to shared types.

### Alternatives Considered

#### Alternative 1: Canonical payload beside the Jev request

**Description:**

Build a backend-neutral payload and derive each backend's request from it.

**Pros:**

- Backends share one translation step.

**Cons:**

- Two forms of one request must be kept equal.
- Shared types learn backend detail.