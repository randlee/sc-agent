# Requirement: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0018: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Proposed  

### Requirement Statement

Definitions:

- The consumer template is the sc-compose template `sc_agent/templates/consumer.py.j2`, which ships inside the sc-decidr wheel. It is rendered with sc-compose 1.8.0 or newer, an external tool that is not a dependency of either wheel.
- sc-decidr and sitr are version-locked: they are released together at the same version. Mixed versions are not supported; this is documented, not enforced.
- The consumer module is the Python file a consumer renders from the consumer template.
- A feature variable is a template variable that selects a section of the consumer module: `logging` (`general`, `session` or `both`), `models` (the model selectors the consumer offers, passed through unchanged) and `sitr` (`true` or `false`).
- The public API is `send`, `send_batch`, `category`, `retryable`, `ERROR_IDS` and `SagError`, as REQ-SAG-0016, REQ-SAG-0017 and REQ-SAG-0702 define them, plus `sc_treesitter` for the sitr section.
- The logging wiring is the consumer module's code that passes the consumer's general logger and session log to the public API, as the `logging` feature variable selects.
- The error mapping is the consumer module's code that maps an error to its category with `category`, and that category to a consumer code through a category-to-code map the consumer passes in.
- The sitr section is the part of the consumer module that runs tree-sitter extraction; its output is passed into the consumer's prompt authoring.
- Consumer logic is prompt authoring and the processing of results and decision trees. It stays in the consumer.
- The wheel README is `crates/sc-agent-py/README.md`.
- The keyring overrides are the `keyring_service` and `keyring_account` arguments of `send` and `send_batch`, which choose the keyring entry the wheel reads the API key from.
- A matched release is an sc-decidr wheel and a sitr wheel of the same version.
- A decision call is a call to `send` or `send_batch`.
- A covered release is an sc-agent release of version 0.2.0 or later.
- PyPI is the Python Package Index, pypi.org.

**MUST statements:**

- The consumer template MUST ship in the sc-decidr wheel as package data.
- The consumer module MUST make the decision call through `send` or `send_batch`.
- The consumer module MUST pass the model selector through unchanged.
- Every decision call in the consumer module MUST pass the keyring overrides.
- The consumer module's source MUST contain the logging wiring.
- The consumer module MUST contain the error mapping.
- The consumer module's header MUST record the sc-decidr version that rendered it.
- When `sitr` is true, the consumer module's header MUST record the sitr version.
- Feature variables MUST be the only template input that changes the consumer module's code.
- The consumer template MUST be tested against a matched release.
- PyPI MUST hold the matched release of every covered release.
- The sitr section MUST import `sc_treesitter` lazily, inside that section.
- The sitr section MUST stay separate from the decision call.
- The consumer module MUST call only the public API and the Python standard library.
- Every sc-decidr call in the consumer module MUST also be callable directly by a consumer through the public API.
- The public API MUST stay documented in `sc_agent.pyi`.
- The public API MUST stay documented in the wheel README.
- The wheel README MUST name the consumer template.
- The wheel README MUST give the consumer template's render command.

**MUST NOT statements:**

- The consumer module MUST NOT contain consumer logic.
- The consumer module MUST NOT set `provider`.
- The consumer module MUST NOT parse error message text.
- The consumer module MUST NOT check the recorded versions at runtime.

### Rationale

The owner requires judge-jev, plan-anchor and every later Python consumer to integrate identically for errors, API keys, model selection (the jev, haiku, luna, decider and laya alternates), the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context). One template rendered by every consumer removes that divergence. Shipping the template as package data makes its version the installed wheel's version. Leaving `provider` unset keeps the jev, haiku, luna, decider and laya alternates available. The header's recorded versions are troubleshooting information only.

### Success Criteria

**Acceptance Criteria:**

- [ ] A sc-agent test renders the consumer template with every combination of feature variables and runs each rendered module against the loopback runner (`crates/sc-agent-py/tests/loopback.py`):
  - a success returns the response;
  - an error returns the consumer code from the passed map for its category;
  - with `logging` set to `session` or `both`, the session log has one record per query and response;
  - with `logging` set to `general` or `both`, the injected general logger receives the call's events.
- [ ] A rendered module's header records the installed sc-decidr version, and the sitr version when `sitr` is true, and importing it with a different installed version does not raise.
- [ ] sc-decidr 0.2.0 and sitr 0.2.0 are both installable from PyPI.
- [ ] An offline test renders with `sitr` true: extraction output from a checked-in source file reaches a dry-run request's prompt.
- [ ] A rendered module with `sitr` false imports with `sc_treesitter` absent.
- [ ] Scanning the rendered modules finds only public-API and standard-library references.
- [ ] The README of `crates/sc-agent-py` names the template and its render command, and the consumer CI check (regenerate and diff) is documented there.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- NFR-SAG-0003
