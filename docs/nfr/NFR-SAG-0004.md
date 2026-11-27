# Requirement: One owner for every piece of logic and data; everything else calls it

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## NFR-SAG-0004: One owner for every piece of logic and data; everything else calls it

**Status:** Approved  

### Requirement Statement

Definitions:

- sc-agent is this repository: its Rust crates, the sc-decidr wheel and its scripts.
- A shared fact is a piece of logic or data that more than one program needs, such as the provider table, the provider-key list, the model table, the session-log layout or ULID minting.
- The owner of a shared fact is the one function, constant or table that holds it.
- A copy is a second implementation of a shared fact, or a literal list or table that repeats an owner's values. A test fixture that asserts an owner's output is not a copy.
- A consumer is a program built on the sc-decidr wheel, such as a module rendered from the consumer template (REQ-SAG-0018).

**MUST statements:**

- Every shared fact MUST have a single owner.
- Code that needs a shared fact MUST obtain it by calling its owner.

**MUST NOT statements:**

- sc-agent MUST NOT contain a copy.
- The consumer template MUST NOT contain a copy.
- A consumer MUST NOT contain a copy.

### Rationale

Two consumers each rebuilt the wheel boundary and the copies diverged (ADR-SAG-0019, Context). The first phase-j plan copied the provider-key list into four places, re-implemented the session-log layout in template Python and minted ULIDs in both Rust and Python. One owner per fact keeps one place to change and removes divergence.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test in the sc-agent Rust crate asserts that each listed shared fact (provider table, provider-key list, model table, session-log layout, ULID minting) is defined by one owner.
- [ ] A unit test calls the sc-decidr wheel's provider-key list, session-row reader and `run_id` minting and asserts each returns the value of its owner in the sc-agent Rust crate.
- [ ] Inspection: `rg -n` over sc-agent for the provider-key names, the session-log path layout and ULID minting, outside test fixtures, returns matches only in their owners.
- [ ] Inspection: `rg -n` over `sc_agent/templates/consumer.py.j2` for the provider-key names, the session-log path layout and ULID minting returns no match.
- [ ] Inspection: `rg -n` over a consumer module rendered from `sc_agent/templates/consumer.py.j2` for the provider-key names, the session-log path layout and ULID minting returns no match.

### Dependencies

**Related:**

- ADR-SAG-0019
- ADR-SAG-0021
- REQ-SAG-0018
