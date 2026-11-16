# Requirement: Return values are not rounded

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0007: Return values are not rounded

**Status:** Approved  

### Requirement Statement

Definitions: A decision value is a value a decision model returns (a Jev probability, confidence or score) or a value sc-agent computes from one. A transport timing hint, such as `retry_after_ms` in an error detail, is not a decision value; its owning ADR, ADR-SAG-0019, states how it is rounded.

**MUST statements:**

- sc-agent MUST return decision values at the precision it received or computed them, using floating-point types where values are fractional.
- sc-agent MUST keep the exact value of a transport timing hint available where its owning ADR states.

**MUST NOT statements:**

- sc-agent MUST NOT round any decision value it returns.

### Rationale

Rounding is the consumer's concern. A value rounded inside sc-agent loses information no consumer can recover.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Jev probability of 0.123456789 is returned as 0.123456789 by the Rust API, the CLI and the PyO3 bindings.
- [ ] A workspace search finds no rounding of returned decision values.