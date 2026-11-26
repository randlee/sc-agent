# Requirement: Return values are not rounded

**Status:** Approved  
**Version:** 0.3.2  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0007: Return values are not rounded

**Status:** Approved  

### Requirement Statement

Definitions: A returned float is any floating-point value sc-agent returns, such as a Jev probability, confidence or score.

**MUST NOT statements:**

- sc-agent MUST NOT round a returned float.
- sc-agent MUST NOT convert a returned float to an integer type.

### Rationale

This requirement stops an implementation from rounding or casting a value and so hiding information from the end user. Rounding is the consumer's concern: a value rounded inside sc-agent loses information no consumer can recover.

Owner ruling (2026-11-21), Rand Lee: "I would never create a requirement requiring subnanosecond rounding for a http call." "Don't round % to int."

### Success Criteria

**Acceptance Criteria:**

- [ ] Rust API: a unit test sends a request whose Jev probability is 0.123456789 to a stub server and asserts `send` returns 0.123456789 exactly.
- [ ] CLI: a CLI run of `decidr send` against a stub server whose Jev probability is 0.123456789 prints 0.123456789 exactly in its JSON output.
- [ ] Python bindings: a Python test calls `send` against a stub server whose Jev probability is 0.123456789 and asserts the returned value is 0.123456789 exactly.
- [ ] Inspection: `rg -n 'f32|Decimal|i64' crates/sc-agent/src` shows no fractional decision value held in a type other than `f64`.
- [ ] A unit test passes a Jev confidence of 0.875 through sc-agent and asserts the returned value is 0.875, not 1 or 0.88.
- [ ] Inspection: `rg -n 'round' crates/` finds no rounding of a returned decision value.
