# Requirement: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  
**Version:** 1.7.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0018: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  

### Requirement Statement

Definitions (template contents: ADR-SAG-0019 decision 1):

- The consumer template is `sc_agent/templates/consumer.py.j2`; a consumer renders its consumer module from it; a tool is such a consumer.
- The Run is `sc_agent.Run` (ADR-SAG-0019 decision 14); a decision call is its `send` or `send_batch`. A run is one execution of one consumer command; `run_id` is its ULID.
- The session tags are the Run's `project` and each call's `agent_role`. The session log holds every query and response; a session row is one entry in it.
- The logging wiring gives one Logger (`logger(client name)` or the consumer's) to every Run and sitr call. The error mapping maps an error's `category` to a consumer code through the consumer's map.
- The consent gate checks remote-judge consent before a send. The sitr section runs tree-sitter extraction. Consumer logic is prompt authoring and result handling.
- The shared consumer code is fixture loading and coverage, batch settings and ids, result-line encoding and the consent gate. Extension points are template variables and named blocks.
- The public API is the wheel's names that `sc_agent/__init__.pyi` documents, plus `sc_agent.sitr`; the permitted API adds the standard library and the consumer's modules named through extension points. The wheel README is `crates/sc-agent-py/README.md`. Covered releases are 0.2.0 and later. A copy: NFR-SAG-0004.
- A result line is the JSON object a consumer writes per batch query. A tool record is a consumer's own record. A dry run composes requests without sending.

**MUST statements:**

- The consumer template MUST ship in the sc-decidr wheel as package data.
- The consumer module MUST make every decision call through a Run.
- The consumer module MUST pass its caller's model selector unchanged to the Run.
- Every decision call in the consumer module MUST pass the session tags.
- The consumer module MUST contain the logging wiring.
- The consumer module MUST contain the error mapping.
- Extension points MUST be the only template inputs that change the consumer module's code.
- The consumer template MUST carry the shared consumer code.
- A consumer-specific value MUST reach the consumer module only through an extension point.
- The consumer template MUST be tested against the sc-decidr wheel of the same release.
- PyPI MUST hold the sc-decidr wheel of every covered release.
- The consumer module MUST use `sc_agent.sitr` only inside the sitr section.
- The sitr section MUST stay separate from the decision call.
- The consumer module MUST call only the permitted API.
- The consumer module MUST cancel the running Run on `KeyboardInterrupt`.
- The public API MUST stay documented in `sc_agent/__init__.pyi`.
- The public API MUST stay documented in the wheel README.
- The wheel README MUST name the consumer template.
- The wheel README MUST give the consumer template's render command.
- The consumer module MUST create one Run per run.
- The consumer module MUST close its Run after the run's last decision call.
- Every result line MUST carry the schema tag `jev-batch-result/2`.
- Every result line MUST carry its Run's `run_id`.
- Every result line MUST carry the `session_id` its send returned.
- Every tool record derived from a query MUST carry the run's `run_id`.
- Every tool record derived from a query MUST carry that query's `session_id`.
- The consumer module MUST pass the caller's result-cache setting (REQ-SAG-0703) to its Run.
- A dry run MUST check that its input and arguments compose into a valid request.
- When `TYPESAFE_API_KEY` is absent, every tool MUST complete a decision call with `--model haiku`.
- When `TYPESAFE_API_KEY` is absent, every tool MUST complete a decision call with `--model luna`.
- The consent gate MUST ask for consent only when the Run's provider is TypeSafe.

**MUST NOT statements:**

- The consumer module MUST NOT contain consumer logic.
- The consumer module MUST NOT name a model in code.
- The consumer module MUST NOT choose a provider.
- The consumer module MUST NOT parse error message text.
- The consumer template MUST NOT contain a copy.
- The consumer module MUST NOT mint a ULID.
- A result line MUST NOT carry a tool's own ids.
- A result line MUST NOT repeat data the session row holds.
- A tool record MUST NOT repeat data the session row holds.
- A general-log record MUST NOT repeat data the session row holds.
- A dry run MUST NOT write a record.

### Rationale

The owner requires every Python consumer to integrate identically; consumers differ only in prompt authoring and in processing results and decision trees. One shipped template replaces diverged hand-copied integration (ADR-SAG-0019, Context).

### Success Criteria

**Acceptance Criteria:** (both consumers' renders, on `crates/sc-agent-py/tests/loopback.py`)

- [ ] A unit test finds the consumer template in the built wheel's package data.
- [ ] A unit test scans both renders and finds every `send` and `send_batch` call made on a Run.
- [ ] A loopback test passes `haiku` to a render and finds `haiku` as the Run's model selector.
- [ ] A loopback test finds the call's `project` and `agent_role` on its session-log record.
- [ ] A loopback test finds the call's events in the injected Logger.
- [ ] A loopback test returns the mapped consumer code for an error's category.
- [ ] A unit test renders twice with the same extension-point values and other inputs changed, and finds identical code.
- [ ] A unit test finds fixture loading, batch settings, result-line encoding and the consent gate in the template.
- [ ] A unit test finds no plan-anchor or judge-jev value in the template outside its extension points.
- [ ] A CI run renders the template against the wheel built from the same commit.
- [ ] A CI run installs the released sc-decidr wheel from PyPI.
- [ ] A unit test scans both renders and finds `sc_agent.sitr` only inside the sitr section.
- [ ] A unit test scans the `sitr` true render and finds no decision call inside the sitr section.
- [ ] A unit test scans both renders and finds only permitted-API references.
- [ ] A loopback test raises `KeyboardInterrupt` during a batch and finds its Run cancelled.
- [ ] A unit test finds every public-API name in `sc_agent/__init__.pyi`.
- [ ] A unit test finds every public-API name in the wheel README.
- [ ] A unit test finds the consumer template's path in the wheel README.
- [ ] A unit test finds the render command in the wheel README.
- [ ] A loopback test of a batch of 3 finds one Run created.
- [ ] A loopback test finds the Run closed after the batch's last call.
- [ ] A loopback test of a batch of 3 finds `jev-batch-result/2` on all 3 result lines.
- [ ] A loopback test of a batch of 3 finds the Run's `run_id` on all 3 result lines.
- [ ] A loopback test finds each result line's `session_id` in the session log.
- [ ] A loopback test finds the Run's `run_id` on each tool record.
- [ ] A loopback test finds each tool record's `session_id` on that query's session row.
- [ ] A loopback test runs twice with the cache off and finds a new session row on the second run.
- [ ] A unit test gives a dry run a malformed input and finds it reported invalid.
- [ ] A CLI run of each tool with `TYPESAFE_API_KEY` unset and `--model haiku` completes.
- [ ] A CLI run of each tool with `TYPESAFE_API_KEY` unset and `--model luna` completes.
- [ ] A loopback test finds consent asked for a TypeSafe Run and not for a `haiku` Run.
- [ ] A unit test finds no prompt text or result handling in the template outside its extension points.
- [ ] A unit test scans both renders and finds no model name.
- [ ] A unit test scans both renders and finds no `provider` argument.
- [ ] A unit test scans both renders and finds no read of an error's message.
- [ ] A unit test finds no copy (NFR-SAG-0004) in the template.
- [ ] A unit test scans both renders and finds no ULID creation.
- [ ] A loopback test finds no tool id field on a result line.
- [ ] A loopback test finds no session-row field on a result line.
- [ ] A loopback test finds no session-row field on a tool record.
- [ ] A loopback test finds no session-row field in the general log.
- [ ] A unit test finds no new session row, result line, tool record, link record or general-log record after a dry run.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- REQ-SAG-0703
- NFR-SAG-0003
- NFR-SAG-0004
- REQ-SAG-0005
