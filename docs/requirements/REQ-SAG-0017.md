# Requirement: Every error has a stable id and a stored category, and busy and network errors are retryable

**Status:** Approved  
**Version:** 0.5.2  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error has a stable id and a stored category, and busy and network errors are retryable

**Status:** Approved  

### Requirement Statement

Definitions:

- A busy error and a network error are errors in the `busy` and `network` categories, respectively. The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.
- Retryable means worth resubmitting as a new call later, after the wheel's own resends have run out. It is not a signal for an immediate retry loop.

**MUST statements:**

- Busy errors MUST say they are worth retrying later.
- Network errors MUST say they are worth retrying later.
- Every error value MUST store its category.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today. A category stored when the error is built removes both problems without a lookup.

Owner ruling (2026-11-21), Rand Lee: "always return descriminated unions w/ error info" "No hand built checks."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that the stored `category` is:

  | Error | Category |
  | --- | --- |
  | `http.provider_busy` | `busy` |
  | `http.network` | `network` |

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
