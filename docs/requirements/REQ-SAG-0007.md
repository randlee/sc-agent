# Requirement: Return values are not rounded

**Status:** Approved  
**Version:** 0.3.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
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

- [ ] A unit test passes a Jev probability of 0.123456789 through sc-agent and asserts the returned value equals 0.123456789 exactly.
- [ ] A unit test asserts the Rust type of each fractional decision value field is `f64`.
- [ ] A unit test passes a Jev confidence of 0.875 through sc-agent and asserts the returned value is 0.875, not 1 or 0.88.
