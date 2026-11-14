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

- Each variant's id and category rule MUST be written once, in the `error_catalog!` table from which `id()` and `category_rule()` are generated.
- A variant with no row MUST fail to compile.
- Every catalog row MUST have the base category or the `inherit` rule this table gives it:

  | Category | Ids |
  | --- | --- |
  | busy | `http.provider_busy` |
  | quota | `http.quota_exhausted` |
  | auth | `decidr.no_provider_key`, `secrets.key_missing`, `secrets.key_malformed`, `secrets.keyring_read`, `secrets.keyring_timeout`, `secrets.keyring_construction_refused` |
  | network | `http.network`, `local.load_timeout` |
  | response_invalid | `decision.response_parse`, `decision.schema_mismatch`, `http.response_too_large`, `llm.reply_invalid`, `llm.no_text`, `llm.truncated`, `llm.output_not_json`, `decision_llm.answer_ids`, `decision_llm.answer_invalid`, `local.probe_parse` |
  | validation | `decision.request_parse`, `decision.duplicate_key`, `decision.image_parts`, `decision.limits`, `decision.api_bad_request`, `decision.api_validation`, `decision_llm.invalid_max_tokens`, `decidr.usage`, `decidr.request_invalid`, `decidr.limit_exceeded`, `decidr.input_not_found`, `decidr.image_format`, `decidr.invalid_request_timeout`, `decidr.invalid_workers` (new), `http.invalid_request`, `log.invalid_slug`, `types.invalid_model_id`, `types.unknown_short_name`, `types.unknown_provider`, `types.invalid_var_name`, `types.invalid_session_id`, `sc_agent_py.invalid_option` |
  | cancelled | `decision.cancelled`, `decidr.signal` |
  | config | `config.*` (all eight), `decidr.unknown_runner`, `local.model_not_served`, `http.invalid_endpoint`, `http.key_on_plain_http`, `secrets.invalid_location`, `secrets.dotenv_read`, `types.invalid_release_date`, `types.model_on_two_lines`, `runner.invalid_config` |
  | log | `log.*` except `log.invalid_slug` |
  | other | `decision.api_status`, `llm.api_status`, `local.probe_status` (status-refined), `llm.refused`, `decision.encode`, `decision_llm.encode`, `decision_llm.invariant`, `decision.shutdown`, `decidr.encode`, `decidr.write`, `decidr.input_read`, `decidr.image_read`, `sc_agent_py.worker_panicked` |
  | inherit | `decision.http`, `decision.backend`, `llm.http`, `decision_llm.llm`, `local.http`, `local.decision`, `runner.session_log`, `decidr.config`, `decidr.types`, `decidr.secrets`, `decidr.log`, `decidr.local`, `decidr.decision`, `decidr.http`, `decidr.llm`, `decidr.runner` |
- `sc-agent-types` MUST NOT offer a category function without rows. It MUST offer `ErrorDetail::category_with(rows)`.
- `decidr::category(&ErrorDetail)`, equal to `category_with(error_catalog())`, MUST be the single entry point for in-memory and read-back details alike, and MUST:
  - return the category of the deciding level, the first level from the top whose rule is base, which is the level whose fields the top carries (REQ-SAG-0016);
  - refine `llm.api_status`, `decision.api_status` and `local.probe_status` by that level's status:
    - `auth` for 401 or 403;
    - `quota` for 402;
    - `validation` for 400 or 422;
    - `busy` for 429, 500, 503 or 504;
    - otherwise the base category;
  - return `other` when the chain has no catalog id.
- The rule MUST live in Rust only. sc-agent-py MUST expose it unchanged, through `decidr::category`:
  - `sc_agent.category(detail_or_id, status=None)` and `SagError.category`;
  - `sc_agent.retryable(detail_or_id)`, true for `busy` and `network`. It MUST be documented as "worth resubmitting as a new call later, after the wheel's own resends have run out", not as a signal for an immediate retry loop;
  - `sc_agent.ERROR_IDS`, a `frozenset[str]` equal to the catalog.
- `category` MUST return `other` for an id outside the catalog and MUST NOT raise for it.

**MUST NOT statements:**

- The catalog MUST NOT contain an id twice.
- Categories MUST NOT depend on a global registry, a copied catalog table or a new JSON field.
- The catalog MUST NOT contain ids of release or prerelease tooling; `decidr.integration_evidence` is not a catalog id, and that tooling MUST NOT be part of the published decidr crate or the wheel.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today, and the top-level id of a failure is usually a wrapper. A generated catalog with a category computed over the chain removes both problems.

### Success Criteria

**Acceptance Criteria:**

- [ ] A `compile_fail` doctest shows that an enum with a variant missing from its `error_catalog!` table does not compile.
- [ ] Each crate's sample values of every variant map to ids in that enum's catalog rows.
- [ ] `decidr::error_catalog()` has no duplicate id.
- [ ] `decidr::category` returns:

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
- [ ] A `decision.http` over `ProviderBusy` detail read back from JSON gets `busy` from `decidr::category`.
- [ ] `sc_agent.category("io.ConnectionRefused") == "other"`.
- [ ] The published decidr crate exports no `integration_cases` or `validate_integration_results`, and the wheel has no `sc_agent.integration_cases` or `sc_agent.validate_integration_results`.
- [ ] `sc_agent.ERROR_IDS` equals the catalog (a plumbing check).

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
