# Requirement: Preserve returned decision values

**Status:** Approved  
**Version:** 0.3.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0007: Preserve returned decision values

**Status:** Approved  

### Requirement Statement

Definitions: A decision value is a value a decision model returns (a Jev probability, confidence or score) or a value sc-agent computes from one. A transport timing hint, such as `retry_after` in an error detail, is not a decision value; it is kept as a Duration (ADR-SAG-0019).

**MUST NOT statements:**

- sc-agent MUST NOT round any decision value it returns.
- sc-agent MUST NOT cast any decision value it returns to an integer.

### Rationale

Consumers need the decision values returned by the model. Rounding or casting a value to an integer inside sc-agent can lose information no consumer can recover.

Owner ruling (2026-11-21), Rand Lee: "I would never create a requirement requiring subnanosecond rounding for a http call." "Don't round % to int."

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that a Jev probability of 0.123456789 is returned as 0.123456789 by the Rust API, the CLI and the PyO3 bindings.
- [ ] A unit test verifies that returned decision values are neither rounded nor cast to integers.
