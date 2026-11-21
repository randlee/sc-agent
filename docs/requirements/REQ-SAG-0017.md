# Requirement: Every error has a stable id and a stored category, and busy and network errors are retryable

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error has a stable id and a stored category, and busy and network errors are retryable

**Status:** Approved  

### Requirement Statement

Definitions:

- The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.
- A wrapper is a variant whose cause is a whole lower error. Its detail takes its cause's category (REQ-SAG-0016).
- A leaf variant is a variant that is not a wrapper.
- Retryable means worth resubmitting as a new call later, after the wheel's own resends have run out. It is not a signal for an immediate retry loop.

**MUST statements:**

- Every error MUST have a stable id.
- Busy and network errors MUST say they are worth retrying later.
- The detail of a leaf variant MUST store its category.
- `sc_agent.category(detail)` MUST return the stored category of the top-level detail.
- `sc_agent.category(detail)` MUST return `other` for a detail with no stored category.
- `sc_agent.retryable(detail)` MUST return true for the categories `busy` and `network` only.

**MUST NOT statements:**

- `sc_agent.category(detail)` MUST NOT raise for a detail with no stored category.
- The published decidr crate MUST NOT export `integration_cases` or `validate_integration_results`.
- The sc-decidr wheel MUST NOT export `integration_cases` or `validate_integration_results`.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today, and the top-level id of a failure is usually a wrapper. A category stored when the detail is built, and copied up by wrappers, removes both problems without a lookup across crates.

NFR-SAG-0002 is withdrawn (2026-11-20). Rand Lee: "schema isn't locked yet ... No hand built checks."

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "always return descriminated unions w/ error info" "No hand built checks."

### Success Criteria

**Acceptance Criteria:**

- [ ] Every error returned in the tests has an id, and the same error gives the same id each time.
- [ ] The stored `category` is:

  | Detail | Category |
  | --- | --- |
  | `decision.http` over `ProviderBusy` | `busy` |
  | `decision.http` over `Network` | `network` |
- [ ] `sc_agent.retryable` is true for a `busy` and a `network` detail and false for every other category.
- [ ] `sc_agent.category` of a checked-in detail written before `category` existed, which has no stored category, is `"other"`.
- [ ] The published decidr crate exports no `integration_cases` or `validate_integration_results`, and the wheel has no `sc_agent.integration_cases` or `sc_agent.validate_integration_results`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
