# Requirement: Every error id has a category or wrapper designation in its crate's catalog table, and the category is stored in the detail

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error id has a category or wrapper designation in its crate's catalog table, and the category is stored in the detail

**Status:** Proposed  

### Requirement Statement

Definitions:

- A crate's catalog table is the `error_catalog!` table that crate writes for one of its error enums. Each row gives a variant's id and its category or wrapper designation. sc-agent-py's `PyBindingError` has its own table.
- The catalog is every id in the catalog tables of the decidr-reachable workspace crates and sc-agent-py.
- A foreign id, such as `io.ConnectionRefused` or `foreign.source`, is not in the catalog.
- The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.
- A wrapper takes its cause's fields.

**MUST statements:**

- Each variant's id and category or wrapper designation MUST be written once, in its crate's catalog table, from which `id()` is generated.
- A variant with no row MUST fail to compile.
- Every catalog row MUST have the leaf category or wrapper designation given in ADR-SAG-0019 decision 6.
- A leaf's `detail()` MUST store the base category from its own crate's row.
- For `llm.api_status`, `decision.api_status` and `local.probe_status`, `detail()` MUST store the category refined by the level's status:
  - `auth` for 401 or 403;
  - `quota` for 402;
  - `validation` for 400 or 422;
  - `busy` for 429, 500, 503 or 504;
  - otherwise the base category.
- The rule MUST live in Rust only. sc-agent-py MUST read the stored field:
  - `sc_agent.category(detail)`, the top-level detail's stored category;
  - `sc_agent.retryable(detail)`, true for `busy` and `network`. It MUST be documented as "worth resubmitting as a new call later, after the wheel's own resends have run out", not as a signal for an immediate retry loop.
- `category` MUST return `other` for a detail with no stored category and MUST NOT raise for it.

**MUST NOT statements:**

- A catalog table MUST NOT contain an id twice.
- The catalog MUST NOT contain ids of release or prerelease tooling; `decidr.integration_evidence` is not a catalog id, and that tooling MUST NOT be part of the published decidr crate or the wheel.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today, and the top-level id of a failure is usually a wrapper. A category stored when the detail is built, and copied up by wrappers, removes both problems without a lookup across crates.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each crate's sample values of every variant map to ids in that enum's catalog rows.
- [ ] No crate's catalog table has a duplicate id.
- [ ] The stored `category` is:

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
- [ ] `sc_agent.category` of a checked-in 0.1.0 detail, which has no stored category, is `"other"`.
- [ ] The published decidr crate exports no `integration_cases` or `validate_integration_results`, and the wheel has no `sc_agent.integration_cases` or `sc_agent.validate_integration_results`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
- NFR-SAG-0002
