# Decision: Decision backends implement one trait with one request form

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0001: Decision backends implement one trait with one request form

**Status:** Draft  

### Related Documents

**Requirements:**

- REQ-SAG-0201
- REQ-SAG-0006

### Context

sc-agent sends decisions to Jev, to local runners that serve the Jev API, and to LLM backends such as Haiku and Luna.

**Problem Statement:**

Jev, local Jev-compatible runners, Haiku and Luna must answer the same decision so their answers can be compared, without shared crates knowing how any one backend adapts it.

### Decision

Every decision backend (Jev, local Jev-compatible runners, Haiku, Luna) implements one sans-IO `Backend` trait in `sc-decision-schema`: encode a Jev API request into an HTTP request, and decode an HTTP response into a Jev API response. Callers use one sync `Decision` trait (`decide` for one request, `decide_stream` for many). `sc-decision-runner` implements `Decision` for any `Backend` and transport; the queue, worker threads, holdoff, cancellation and session logging live behind it. Each LLM backend adapts the request internally.

**Key Principles:**

- There is one request form: the Jev API request.
- Adaptation knowledge lives only in the backend that adapts.
- Backends do no I/O; the transport is separate, so an async transport can be added without changing backends.
- `decide_stream` returns results fastest first, each tagged with its session ULID; a failed request is returned in its result and the stream continues.

### Rationale

When every backend receives the same data, their answers are comparable by construction, and shared crates stay free of backend-specific types.

### Consequences

**Positive:**

- A new backend is added by implementing the trait, with no change to shared types.

**Negative:**

- Each LLM backend must translate the full Jev request itself.

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