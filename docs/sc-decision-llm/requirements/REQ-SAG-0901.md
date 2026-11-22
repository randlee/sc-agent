# Requirement: LLM decision backends return the Jev schema

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0901: LLM decision backends return the Jev schema

**Status:** Approved  

### Requirement Statement

Definitions: Decider mode is an LLM such as Haiku or Luna answering a decision request in place of a decision model. The required schema is the Jev API answer schema for each question type, including `probabilities` and `confidence` where Jev returns them.

**MUST statements:**

- In decider mode, Haiku and Luna MUST produce the probabilities and confidence themselves and return answers in the required schema.
- Each request MUST ask the provider for the required schema through the provider's structured-output feature.
- A response that does not match the required schema MUST be returned as `decision.schema_mismatch` with what did not match.
- Haiku and Luna MUST pass the request's inputs to the provider; an input the provider rejects MUST return the provider's error.

### Rationale

Callers compare Jev and LLM answers directly, so every backend must return the same answer shape.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Haiku or Luna answer to a choice, score, noul and box question parses as the Jev answer type for that question.
- [ ] A test provider that returns a response missing `probabilities` yields `decision.schema_mismatch`.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201