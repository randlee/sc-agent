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

- sc-agent MUST return each decision value unchanged from the value it received or computed.

**MUST NOT statements:**

- sc-agent MUST NOT round any decision value it returns.
- sc-agent MUST NOT cast any decision value it returns to an integer.

### Rationale

Rounding is the consumer's concern. A value rounded inside sc-agent loses information no consumer can recover.

Owner ruling (2026-11-21), Rand Lee: "I would never create a requirement requiring subnanosecond rounding for a http call." "Don't round % to int."

### Success Criteria

**Acceptance Criteria:**

- [ ] Rust API: a unit test sends a request whose Jev probability is 0.123456789 to a stub server and asserts `send` returns 0.123456789 exactly.
- [ ] CLI: a CLI run of `decidr send` against a stub server whose Jev probability is 0.123456789 prints 0.123456789 exactly in its JSON output.
- [ ] Python bindings: a Python test calls `send` against a stub server whose Jev probability is 0.123456789 and asserts the returned value is 0.123456789 exactly.
- [ ] Inspection: `rg -n 'f32|Decimal|i64' crates/sc-agent/src` shows no fractional decision value held in a type other than `f64`.
- [ ] A unit test passes a Jev confidence of 0.875 through sc-agent and asserts the returned value is 0.875, not 1 or 0.88.
- [ ] Inspection: `rg -n 'round' crates/` finds no rounding of a returned decision value.
