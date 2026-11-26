# Requirement: Every error has a stable id and a stored category, and busy and network errors are retryable

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error has a stable id and a stored category, and busy and network errors are retryable

**Status:** Approved  

### Requirement Statement

Definitions:

- The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.
- Retryable means worth resubmitting as a new call later, after the wheel's own resends have run out. It is not a signal for an immediate retry loop.

**MUST statements:**

- Every error MUST have a stable id.
- Busy and network errors MUST say they are worth retrying later.
- Every error value MUST store its category.

**MUST NOT statements:**

- The published decidr crate MUST NOT export `integration_cases` or `validate_integration_results`.
- The sc-decidr wheel MUST NOT export `integration_cases` or `validate_integration_results`.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today. A category stored when the error is built removes both problems without a lookup.

NFR-SAG-0002 is withdrawn (2026-11-20). Rand Lee: "schema isn't locked yet ... No hand built checks."

Owner ruling (2026-11-21), Rand Lee: "always return descriminated unions w/ error info" "No hand built checks."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] Every error returned in the tests has an id, and the same error gives the same id each time.
- [ ] The stored `category` is:

  | Error | Category |
  | --- | --- |
  | `http.provider_busy` | `busy` |
  | `http.network` | `network` |
- [ ] The published decidr crate exports no `integration_cases` or `validate_integration_results`, and the wheel has no `sc_agent.integration_cases` or `sc_agent.validate_integration_results`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
