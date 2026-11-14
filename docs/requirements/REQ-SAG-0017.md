# Requirement: Every error id is in one catalog with a category, computed over the cause chain

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error id is in one catalog with a category, computed over the cause chain

**Status:** Proposed  

### Requirement Statement

Definitions:

- The catalog is every id that a decidr-reachable workspace error enum can return, including sc-agent-py's binding errors.
- A foreign id, such as `io.ConnectionRefused` or `foreign.source`, is not in the catalog.
- The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.
- A wrapper id has the rule `inherit`.

**MUST statements:**

- Each variant's id and category rule MUST be written once, in the `error_catalog!` table from which `id()` is generated.
- A variant with no row MUST fail to compile.
- Every catalog row MUST have the base category or the `inherit` rule given in ADR-SAG-0019 decision 6.
- `ErrorDetail::category()` MUST:
  - return the category of the deciding level, the first level from the top whose rule is base, which is the level whose fields the top carries (REQ-SAG-0016);
  - refine `llm.api_status`, `decision.api_status` and `local.probe_status` by that level's status:
    - `auth` for 401 or 403;
    - `quota` for 402;
    - `validation` for 400 or 422;
    - `busy` for 429, 500, 503 or 504;
    - otherwise the base category;
  - return `other` when the chain has no catalog id.
- The rule MUST live in Rust only. sc-agent-py MUST expose it unchanged:
  - `sc_agent.category(detail_or_id, status=None)`;
  - `sc_agent.retryable(detail_or_id)`, true for `busy` and `network`. It MUST be documented as "worth resubmitting as a new call later, after the wheel's own resends have run out", not as a signal for an immediate retry loop;
  - `sc_agent.ERROR_IDS`, a `frozenset[str]` equal to the catalog.
- `category` MUST return `other` for an id outside the catalog and MUST NOT raise for it.

**MUST NOT statements:**

- The catalog MUST NOT contain an id twice.
- The catalog MUST NOT contain ids of release or prerelease tooling; `decidr.integration_evidence` is not a catalog id, and that tooling MUST NOT be part of the published decidr crate or the wheel.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today, and the top-level id of a failure is usually a wrapper. A generated catalog with a category computed over the chain removes both problems.

### Success Criteria

**Acceptance Criteria:**

- [ ] A `compile_fail` doctest shows that an enum with a variant missing from its `error_catalog!` table does not compile.
- [ ] Each crate's sample values of every variant map to ids in that enum's catalog rows.
- [ ] `decidr::error_catalog()` has no duplicate id.
- [ ] `category()` returns:

  | Detail | Category |
  | --- | --- |
  | `decision.http` over `ProviderBusy` | `busy` |
  | `decision.http` over `QuotaExhausted` | `quota` |
  | `decision.http` over `Network` | `network` |
  | `decision.backend` over a 401 `llm.api_status` | `auth` |
  | `local.probe_status` with 429 | `busy` |
  | `decision.api_status` with 402 | `quota` |
  | `decision.api_status` with 500, 503 or 504 | `busy` |
  | `decision.api_status` with 501 | `other` |
- [ ] `sc_agent.category("io.ConnectionRefused") == "other"`.
- [ ] The published decidr crate exports no `integration_cases` or `validate_integration_results`, and the wheel has no `sc_agent.integration_cases` or `sc_agent.validate_integration_results`.
- [ ] `sc_agent.ERROR_IDS` equals the catalog (a plumbing check).

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
