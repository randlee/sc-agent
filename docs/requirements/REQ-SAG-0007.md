# Requirement: Return values are not rounded

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0007: Return values are not rounded

**Status:** Approved  

### Requirement Statement

Definitions: A decision value is a value a decision model returns (a Jev probability, confidence or score) or a value sc-agent computes from one. A transport timing hint, such as `retry_after` in an error detail, is not a decision value; it is kept as a Duration (ADR-SAG-0019).

**MUST statements:**

- sc-agent MUST return decision values at the precision it received or computed them.
- sc-agent MUST use floating-point types for fractional decision values.

**MUST NOT statements:**

- sc-agent MUST NOT round any decision value it returns.

### Rationale

Rounding is the consumer's concern. A value rounded inside sc-agent loses information no consumer can recover.

Owner ruling (2026-11-21), Rand Lee: "I would never create a requirement requiring subnanosecond rounding for a http call." "Don't round % to int."

### Success Criteria

**Acceptance Criteria:**

- [ ] A Jev probability of 0.123456789 is returned as 0.123456789 by the Rust API, the CLI and the PyO3 bindings.
- [ ] A workspace search finds no rounding of returned decision values.
