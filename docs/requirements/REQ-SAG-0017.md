# Requirement: Every error id has a category or wrapper designation in its crate's catalog table, and the category is stored in the detail

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0017: Every error id has a category or wrapper designation in its crate's catalog table, and the category is stored in the detail

**Status:** Approved  

### Requirement Statement

Definitions:

- A catalog table is the `error_catalog!` table a crate writes for one of its error enums. Each row gives one variant's id and either its category or a wrapper designation. sc-agent-py's `PyBindingError` has its own catalog table.
- The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config` and `other`.
- A wrapper is a variant whose cause is a whole lower error. Its detail takes its cause's category (REQ-SAG-0016).
- A leaf variant is a variant that is not a wrapper.
- A status-refined id is `llm.api_status`, `decision.api_status` or `local.probe_status`.
- The status refinement of an HTTP status is:

  | Status | Category |
  | --- | --- |
  | 401, 403 | `auth` |
  | 402 | `quota` |
  | 400, 422 | `validation` |
  | a 429 with a quota marker (REQ-SAG-0602) | `quota` |
  | any other 429, or any 5xx | `busy` |
  | any other status | `other` |

- Retryable means worth resubmitting as a new call later, after the wheel's own resends have run out. It is not a signal for an immediate retry loop.
- The category table gives the category or wrapper designation of every id:

  | Category | Ids |
  | --- | --- |
  | busy | `http.provider_busy` |
  | quota | `http.quota_exhausted` |
  | auth | `decidr.no_provider_key`, `secrets.key_missing`, `secrets.key_malformed`, `secrets.keyring_read`, `secrets.keyring_timeout`, `secrets.keyring_construction_refused` |
  | network | `http.network`, `local.load_timeout` |
  | response_invalid | `decision.response_parse`, `decision.schema_mismatch`, `http.response_too_large`, `llm.reply_invalid`, `llm.no_text`, `llm.truncated`, `llm.output_not_json`, `decision_llm.answer_ids`, `decision_llm.answer_invalid`, `local.probe_parse` |
  | validation | `decision.context_exceeded`, `decision.request_parse`, `decision.duplicate_key`, `decision.image_parts`, `decision.limits`, `decision.api_bad_request`, `decision.api_validation`, `decision_llm.invalid_max_tokens`, `decidr.usage`, `decidr.request_invalid`, `decidr.limit_exceeded`, `decidr.input_not_found`, `decidr.image_format`, `decidr.invalid_request_timeout`, `decidr.invalid_workers`, `http.invalid_request`, `log.invalid_slug`, `types.invalid_model_id`, `types.unknown_short_name`, `types.unknown_provider`, `types.invalid_var_name`, `types.invalid_session_id`, `sc_agent_py.invalid_option` |
  | cancelled | `decision.cancelled`, `decidr.signal` |
  | config | `decidr.ambiguous_keyring_override`, every `config.*` id, `decidr.unknown_runner`, `local.model_not_served`, `http.invalid_endpoint`, `http.key_on_plain_http`, `secrets.invalid_location`, `secrets.dotenv_read`, `types.invalid_release_date`, `types.model_on_two_lines`, `runner.invalid_config` |
  | other | the status-refined ids, `llm.refused`, `decision.encode`, `decision_llm.encode`, `decision_llm.invariant`, `decision.shutdown`, `decidr.encode`, `decidr.write`, `decidr.input_read`, `decidr.image_read`, `sc_agent_py.worker_panicked` |
  | wrapper | `decision.http`, `decision.backend`, `llm.http`, `decision_llm.llm`, `local.http`, `local.decision`, `runner.session_log`, `decidr.config`, `decidr.types`, `decidr.secrets`, `decidr.log`, `decidr.local`, `decidr.decision`, `decidr.http`, `decidr.llm`, `decidr.runner` |

**MUST statements:**

- Every error variant MUST have exactly one row in its crate's catalog table.
- `ErrorId::id()` MUST return the id in the variant's catalog row.
- An error variant with no catalog row MUST fail to compile.
- Every catalog row MUST have the category or wrapper designation that the category table gives its id.
- The detail of a leaf variant whose id is not status-refined MUST store the category in the variant's catalog row.
- The detail of a status-refined id MUST store the status refinement of its status.
- The status refinement MUST be implemented in Rust only.
- `sc_agent.category(detail)` MUST return the stored category of the top-level detail.
- `sc_agent.category(detail)` MUST return `other` for a detail with no stored category.
- `sc_agent.retryable(detail)` MUST return true for the categories `busy` and `network` only.

**MUST NOT statements:**

- A catalog table MUST NOT contain an id twice.
- `sc_agent.category(detail)` MUST NOT raise for a detail with no stored category.
- A catalog table MUST NOT contain `decidr.integration_evidence`.
- The published decidr crate MUST NOT export `integration_cases` or `validate_integration_results`.
- The sc-decidr wheel MUST NOT export `integration_cases` or `validate_integration_results`.

### Rationale

Consumers that branch on error ids pin their own lists and add fallbacks today, and the top-level id of a failure is usually a wrapper. A category stored when the detail is built, and copied up by wrappers, removes both problems without a lookup across crates.

NFR-SAG-0002 is withdrawn (2026-11-20). Rand Lee: "schema isn't locked yet ... No hand built checks."

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
  | `decision.api_status` with 500, 501, 503 or 504 | `busy` |
  | `decision.api_status` with 418 | `other` |
- [ ] `sc_agent.category` of a checked-in detail written before `category` existed, which has no stored category, is `"other"`.
- [ ] The published decidr crate exports no `integration_cases` or `validate_integration_results`, and the wheel has no `sc_agent.integration_cases` or `sc_agent.validate_integration_results`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0016
