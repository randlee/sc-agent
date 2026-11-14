# Requirement: Every error id is published in a catalog with one category

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error id is published in a catalog with one category

**Status:** Proposed  

### Requirement Statement

Definitions: The error-id catalog is the list of every id that a workspace error enum variant (and `sc_agent_py.worker_panicked`) can return. A foreign id, such as `io.ConnectionRefused` or `foreign.source`, comes from outside the workspace and is not in the catalog. The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.

**MUST statements:**

- The catalog MUST be generated from the error enums. Each variant's id and category MUST be written once, in the same table that generates `id()`.
- A variant with no catalog row MUST fail to compile.
- Every catalog id MUST have exactly one base category.
- `sc_agent.ERROR_IDS` MUST be a `frozenset[str]` equal to the catalog.
- `sc_agent.category(id, status=None)` MUST return the id's base category.
- When `status` is given and the id is `llm.api_status`, `decision.api_status` or `local.probe_status`, `category` MUST return:
  - `auth` for 401 or 403;
  - `busy` for 429;
  - `validation` for any other 4xx;
  - otherwise the base category.
- `category` MUST return `other` for an id that is not in the catalog.

**MUST NOT statements:**

- The catalog MUST NOT contain an id twice.

### Rationale

Consumers that branch on error ids today pin their own copies of the list and add fallbacks for ids they have not seen. One generated catalog removes that drift.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust test finds no duplicate id in `decidr::error_catalog()`.
- [ ] A Python test finds that `ERROR_IDS` equals the catalog exposed by the module, and that `category` returns a listed category for every id in it.
- [ ] `category("llm.api_status", 401) == "auth"` and `category("llm.api_status", 429) == "busy"`.
- [ ] `category("decision.api_status", 500)` returns the base category.
- [ ] `category("io.ConnectionRefused") == "other"`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
