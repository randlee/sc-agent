# Decision: Every provider receives one request form

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-26  
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

Every provider (Jev, local Jev-compatible runners, Haiku, Luna) receives the same Jev API request body. `sc-agent` holds one static provider table and one model table made of built-in rows plus rows added from config, so a new model works without waiting for a release (ANC-009; Rand Lee: "hard-coded rows will get stale quickly and often"). Each provider names its wire format: Jev TypeSafe systemone, OpenAI-compatible chat/completions or Anthropic-compatible Messages. Decision providers implement one decision trait, with one implementation per wire format, and every provider that speaks a wire format uses that format's implementation, so adding a provider needs no new code or hard-coding (ANC-012). `sc-agent` also provides one LLM trait for JSON-in/JSON-out calls, with one implementation per wire format (OpenAI-compatible, Anthropic-compatible) shared by every provider speaking that format, so callers reach any LLM through one interface (ANC-013). Rand Lee, 2026-11-26: "there should be implementations of the decison trait for jev, openai-sdk, anthropic-sdk and compatible providers.  we will also need a llm-trait or similar to enable json-in/json-out llm behavior for open-ai-sdk, anthropic-sdk and compatible providers.  i.e. all providers w/ compatible api should use the same impelemntation." Before a send, the decision implementation adapts the request to its wire format; the reply is decoded back into a Jev API response. Callers open one `Run` per command (created, used for its calls, then disposed) and use its two methods, `send` for one request and `send_batch` for many; the worker threads, holdoff, cancellation and session logging live behind them. There is no wrapper request type.

**Key Principles:**

- There is one request form: the Jev API request body.
- Adaptation knowledge lives only in the wire-format implementations.
- `send_batch` returns each result as it is ready, tagged with its session ULID and input position; a failed request is returned in its result and the batch continues.

### Rationale

When every backend receives the same data, their answers are comparable by construction, and shared code stays free of provider-specific types.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling (2026-11-21), Rand Lee: "I do not require results in request order."

### Consequences

**Positive:**

- A new provider that speaks an existing wire format is added as a table row, with no new code; a new model is added as a config row.

**Negative:**

- Each wire-format implementation must translate the full Jev request.

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