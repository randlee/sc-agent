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

- A shared fact is a piece of logic or data that more than one program needs, such as the provider table, the provider-key list, the model table, the session-log layout or ULID minting.
- The owner of a shared fact is the one function, constant or table that holds it.
- A copy is a second implementation of a shared fact, or a literal list or table that repeats an owner's values. A test fixture that asserts an owner's output is not a copy.
- A consumer is a program built on the sc-decidr wheel, such as a module rendered from the consumer template (REQ-SAG-0018).

**MUST statements:**

- Every shared fact MUST have exactly one owner.
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
- [ ] A unit test asserts that the sc-decidr wheel's provider-key list, session-row reader and `run_id` minting return the Rust owners' values, so callers obtain them by calling the owner.
- [ ] A unit test searches sc-agent, outside test fixtures, and finds no literal provider-key name, session-log path layout or ULID minting outside their owners.
- [ ] A unit test searches the consumer template and finds no literal provider-key name, session-log path layout or ULID minting.
- [ ] A unit test renders a consumer module from the template and finds no literal provider-key name, session-log path layout or ULID minting in it.

### Dependencies

**Related:**

- ADR-SAG-0019
- ADR-SAG-0021
- REQ-SAG-0018
- REQ-SAG-0022
