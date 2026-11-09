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

sc-agent sends decisions to Jev and to LLM backends such as Haiku and Luna. The earlier implementation built two forms of the same request, a Jev request and a canonical payload, and spread mock knowledge into shared types.

**Problem Statement:**

Jev, Haiku and Luna must answer the same decision so their answers can be compared, without shared crates knowing how any one backend adapts it.

### Decision

Jev, Haiku and Luna each implement one decision trait. Every backend receives the same Jev API request and returns a Jev API response. Each LLM backend adapts the request internally.

**Key Principles:**

- There is one request form: the Jev API request.
- Adaptation knowledge lives only in the backend that adapts.

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