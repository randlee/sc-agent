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

- A shared fact is a piece of logic or data that more than one program needs, such as the provider table, the provider-key list, the model table, the session-log layout, ULID minting or the log root.
- The owner of a shared fact is the one function, constant or table that holds it. For every shared fact listed in this record the owner is in the sc-agent Rust crate, exposed through the `decidr` CLI and the sc-decidr wheel.
- A copy is a second implementation of a shared fact, or a literal list or table that repeats an owner's values, anywhere in sc-agent or in a consumer of the sc-decidr wheel. A test fixture that asserts an owner's output is not a copy.
- A consumer is a program built on the sc-decidr wheel, such as a module rendered from the consumer template (REQ-SAG-0018).

**MUST statements:**

- Every shared fact MUST have exactly one owner.
- Code that needs a shared fact MUST obtain it by calling its owner.
- The sc-decidr wheel MUST export the provider-key list, the session-row reader and the `Run` object that mints `run_id` (REQ-SAG-0018), so a consumer can call them instead of copying them.

**MUST NOT statements:**

- sc-agent MUST NOT contain a copy.
- The consumer template MUST NOT contain a copy.
- A consumer MUST NOT contain a copy.

### Rationale

Two consumers each rebuilt the wheel boundary and the copies diverged (ADR-SAG-0019, Context). The first phase-j plan copied the provider-key list into four places, re-implemented the session-log layout in template Python and minted ULIDs in both Rust and Python. One owner per fact keeps one place to change and removes divergence.

Owner ruling 2026-11-24: no copying. Every piece of logic or data (provider and key lists, session-log layout, id minting, model tables) has one owner and everything else calls it. QA treats a duplicated implementation or a copied list as blocking.

### Success Criteria

**Acceptance Criteria:**

- [ ] A search of sc-agent and of the rendered consumer modules finds the provider-key names, the session-log path layout and ULID minting only in their owners in the sc-agent Rust crate, outside test fixtures.
- [ ] The consumer template's redaction gets its key list by calling the wheel's provider-key export, and a contract test asserts that export equals the credential names of the Rust provider table.
- [ ] QA reports any copy it finds in a sprint's diff as a blocking finding.

### Dependencies

**Related:**

- ADR-SAG-0019
- ADR-SAG-0021
- REQ-SAG-0018
- REQ-SAG-0022
