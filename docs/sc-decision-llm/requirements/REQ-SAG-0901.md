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

Definitions: Decider mode uses a large language model, such as Haiku or Luna, to answer a decision request in place of a decision model. The covered models are Haiku and Luna. The provider is the service to which the decision request is sent. The required schema is the Jev decision API's answer schema for the requested question type, including `probabilities` and `confidence` where Jev returns them. The provider's structured-output feature lets a request specify the schema the provider's answer must follow. A `decision.schema_mismatch` error identifies what in the provider response failed to match the required schema.

**MUST statements:**

- In decider mode, the covered model answering the request MUST itself generate the `probabilities` value when the required schema includes that field.
- In decider mode, the covered model answering the request MUST itself generate the `confidence` value when the required schema includes that field.
- In decider mode, answers from each covered model MUST conform to the required schema.
- Each decision request MUST specify the required schema through the provider's structured-output feature.
- If a provider response fails to match the required schema, the decision backend MUST return `decision.schema_mismatch`.
- The covered model handling the decision request MUST pass the request's inputs to the provider.
- If the provider rejects a request input, the decision backend MUST return the provider's error.

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