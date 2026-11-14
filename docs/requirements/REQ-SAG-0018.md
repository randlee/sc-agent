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

- The consumer template is the sc-compose template `sc_agent/templates/consumer.py.j2`, which ships inside the sc-decidr wheel.
- The consumer module is the Python file a consumer renders from the consumer template.
- A feature variable is a template variable that selects a section of the consumer module: `logging` (`general`, `session` or `both`), `models` (the model selectors the consumer offers, passed through unchanged) and `sitr` (`true` or `false`).
- The public API is `send`, `send_batch`, `category`, `retryable`, `ERROR_IDS` and `SagError`, as REQ-SAG-0016, REQ-SAG-0017 and REQ-SAG-0702 define them, plus `sc_treesitter` for the sitr section.

**MUST statements:**

- The consumer template MUST ship in the sc-decidr wheel as package data, so its version is the installed wheel's version.
- The consumer module MUST cover:
  - the call, through `send` or `send_batch`;
  - passing the model selector through unchanged, with `provider` left unset;
  - API-key lookup, including the keyring overrides;
  - wiring the general logger and the session log;
  - mapping an error to its category and then to a consumer code, through a category-to-code map the consumer passes in.
- The consumer module's header MUST record the sc-decidr version that rendered it, and the sitr version when `sitr` is true.
- On import, the consumer module MUST raise `RuntimeError` naming both versions when a recorded version differs from the installed sc-decidr or sitr version.
- Feature variables MUST be the only input that changes the consumer module's code; the same variables MUST render byte-identical files.
- The sitr section MUST import `sc_treesitter` lazily, inside that section, and MUST stay separate from the decision call: its output is passed into the consumer's prompt authoring.
- The consumer module MUST call only the public API and the Python standard library.
- Every behavior of the consumer module MUST also be reachable through the public API alone.
- The public API MUST stay documented in `sc_agent.pyi` and the wheel README; the README MUST point to the consumer template and give its render command.

**MUST NOT statements:**

- The consumer module MUST NOT contain prompt authoring or result or decision-tree processing; those stay in the consumer.
- The consumer module MUST NOT set `provider`.
- The consumer module MUST NOT parse error message text.

### Rationale

The owner requires judge-jev, plan-anchor and every later Python consumer to integrate identically for errors, API keys, model selection (the jev, haiku, luna, decider and laya alternates), the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context). One template rendered by every consumer removes that divergence.

### Success Criteria

**Acceptance Criteria:**

- [ ] A sc-agent test renders the consumer template with every combination of feature variables and runs each rendered module against the loopback runner (`crates/sc-agent-py/tests/loopback.py`):
  - a success returns the response;
  - an error returns the consumer code from the passed map for its category;
  - with `logging` set to `session` or `both`, the session log has one record per query and response;
  - with `logging` set to `general` or `both`, the injected general logger receives the call's events.
- [ ] Rendering the same variables twice gives byte-identical files.
- [ ] A rendered module whose header records a version other than the installed one raises `RuntimeError` on import, naming both versions.
- [ ] An offline test renders with `sitr` true: extraction output from a checked-in source file reaches a dry-run request's prompt.
- [ ] A rendered module with `sitr` false imports with `sc_treesitter` absent.
- [ ] Scanning the rendered modules finds only public-API and standard-library references.
- [ ] The README of `crates/sc-agent-py` names the template and its render command, and the consumer CI check (regenerate and diff) is documented there.

### Dependencies

**Related:**

- ADR-SAG-0019 decisions 1, 11 and 12
- REQ-SAG-0016, REQ-SAG-0017, REQ-SAG-0702: the public API the module uses
- NFR-SAG-0003: the Python 3.11 floor the module declares
