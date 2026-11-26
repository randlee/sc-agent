# Requirement: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  
**Version:** 1.8.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0018: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  

### Requirement Statement

Definitions (how the template is built: ADR-SAG-0019 decision 1):

- A consumer is a Python tool that makes Jev decision calls through the sc-decidr wheel; plan-anchor and judge-jev are the first two.
- The consumer template is `sc_agent/templates/consumer.py.j2`. A consumer module is the Python file a consumer renders from it.
- A consumer-specific value is a value that differs between consumers, such as a client name, a prompt or a result format.
- The sc-decidr wheel is the Python wheel that ships the `sc_agent` package.
- An extension point is a template variable or a named block of the consumer template.
- Prompt authoring is building the text sent to the model. Result processing is handling the returned results and decision trees. Consumer logic is prompt authoring together with result processing.
- A decision call is a Run's `send` or `send_batch`.
- The shared consumer code is fixture loading and coverage, batch settings and ids, result-line encoding and the consent gate.
- The public API is the names the sc-decidr wheel exports to consumers.
- The session tags are the `project` and `agent_role` values a decision call records in the session log.
- The permitted API is the names ADR-SAG-0019 decision 1 makes public: `Run` (with `send` and `send_batch`), `session_rows`, `provider_keys`, `TYPESAFE`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr`, plus the Python standard library and the consumer's own modules named through an extension point.
- A dry run composes a consumer's requests without sending them. A record is a session-log row, a result line, a tool record, a link record or a general-log entry.
- The consent gate is the consumer module's `allow_remote_judge` check before a decision call.
- `haiku` and `luna` are model selectors (REQ-SAG-0016).

**MUST statements:**

- Every consumer MUST make its decision calls from code in its consumer module.
- A consumer-specific value MUST reach the consumer module only through an extension point.
- Extension points MUST be the only template inputs that change the consumer module's code.
- The consumer template MUST carry the shared consumer code.
- The consumer template MUST be tested against the sc-decidr wheel of the same release.
- The consumer module MUST call only the permitted API.
- The consumer module MUST pass the model selector through unchanged.
- The sc-decidr wheel's package data MUST include the consumer template.
- Every decision call in the consumer module MUST pass the session tags.
- The consumer module MUST cancel its running Run on `KeyboardInterrupt`.
- A dry run MUST check that its input and arguments compose into a valid request.
- The consent gate MUST ask for consent only when the Run's provider is TypeSafe.

**MUST NOT statements:**

- The consumer module MUST NOT contain consumer logic.
- The consumer module MUST NOT pass a `provider` argument to a decision call.
- The consumer module MUST NOT parse error message text.
- A dry run MUST NOT write any record.

### Rationale

The owner requires every Python consumer to integrate identically, for errors, API keys, model selection, logging and session logging, and to differ only in prompt authoring and in processing results and decision trees. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context); one shipped template removes that divergence. Shipping the template in the wheel makes its version the installed wheel's version.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test scans plan-anchor and judge-jev and finds every decision call inside a module rendered from the consumer template.
- [ ] Inspection of `sc_agent/templates/consumer.py.j2` finds no plan-anchor or judge-jev value outside its extension points.
- [ ] A unit test renders plan-anchor and judge-jev and finds the two modules identical outside their extension points.
- [ ] Inspection of `sc_agent/templates/consumer.py.j2` finds each part of the shared consumer code.
- [ ] A loopback test runs each rendered module against the wheel built from the same commit and gets a success response with one session-log record per query.
- [ ] Scanning both rendered modules finds only wheel references to the permitted API: `Run`, `send`, `send_batch`, `session_rows`, `provider_keys`, `TYPESAFE`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr`.
- [ ] A loopback unit test passes `haiku` to each rendered module and finds `haiku` as the model selector of its decision call.
- [ ] A unit test finds the consumer template in the built wheel's package data.
- [ ] A loopback test finds each query's session-log record carrying the session tags its call was given.
- [ ] A loopback test raises `KeyboardInterrupt` during a batch and finds the Run cancelled.
- [ ] A unit test gives a dry run a malformed input and finds it reported invalid.
- [ ] A loopback test finds consent asked for a TypeSafe Run and not asked for a `haiku` Run.
- [ ] Inspection of `sc_agent/templates/consumer.py.j2` finds no prompt text or result handling outside its extension points.
- [ ] Inspection of `sc_agent/templates/consumer.py.j2` finds no provider argument.
- [ ] Inspection of `sc_agent/templates/consumer.py.j2` finds no read of an error's message.
- [ ] A unit test finds no new record after a dry run.

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
