# Requirement: Return values are not rounded

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0007: Return values are not rounded

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- sc-agent MUST return numeric values at the precision it received or computed them, using floating-point types where values are fractional.

**MUST NOT statements:**

- sc-agent MUST NOT round any value it returns.

### Rationale

Rounding is the consumer's concern. A value rounded inside sc-agent loses information no consumer can recover.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Jev probability of 0.123456789 is returned as 0.123456789 by the Rust API, the CLI and the PyO3 bindings.
- [ ] A workspace search finds no rounding of returned values.