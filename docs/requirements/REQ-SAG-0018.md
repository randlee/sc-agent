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

Definitions (how the template is built: ADR-SAG-0019 decision 1):

- A consumer is a Python tool that makes Jev decision calls through the sc-decidr wheel; plan-anchor and judge-jev are the first two.
- The consumer template is `sc_agent/templates/consumer.py.j2`. A consumer module is the Python file a consumer renders from it.
- A consumer-specific value is a value that differs between consumers, such as a client name, a prompt or a result format.
- The sc-decidr wheel is the Python wheel that ships the `sc_agent` package.
- An extension point is a template variable or a named block of the consumer template.
- Prompt authoring is building the text sent to the model. Result processing is handling the returned results and decision trees. Consumer logic is prompt authoring together with result processing.
- A decision call is a Run's `send` or `send_batch`. The sitr section is the part of a consumer module that runs tree-sitter extraction.
- A covered release is an sc-agent release of version 0.2.0 or later. PyPI is the Python Package Index, pypi.org.
- The shared consumer code is fixture loading and coverage, batch settings and ids, result-line encoding and the consent gate.
- The error mapping maps an error's `category` (REQ-SAG-0017) to a consumer code through the consumer's map.
- The public API is the names the sc-decidr wheel exports to consumers. The wheel README is `crates/sc-agent-py/README.md`.
- The session tags are the `project` and `agent_role` values a decision call records in the session log.
- The logging wiring passes one Logger, the consumer's or `logger(client name)`, to every decision call.
- The permitted API is `send`, `send_batch`, `digest`, `logger`, `log`, `flush`, `Cancel` and `sc_agent.sitr`, plus the Python standard library and the consumer's own modules named through an extension point.
- A dry run composes a consumer's requests without sending them. A record is a session-log row, a result line, a tool record, a link record or a general-log entry.
- The consent gate is the consumer module's `allow_remote_judge` check before a decision call.
- `haiku` and `luna` are model selectors (REQ-SAG-0016). The TypeSafe key is `TYPESAFE_API_KEY`.

**MUST statements:**

- Every consumer MUST make its decision calls inside its consumer module.
- A consumer-specific value MUST reach the consumer module only through an extension point.
- Extension points MUST be the only template inputs that change the consumer module's code.
- The consumer template MUST carry the shared consumer code.
- The consumer module MUST contain the error mapping.
- The consumer template MUST be tested against the sc-decidr wheel of the same release.
- The consumer module MUST call only the permitted API.
- The consumer module MUST pass the model selector through unchanged.
- The sc-decidr wheel's package data MUST include the consumer template.
- PyPI MUST hold the sc-decidr wheel of every covered release.
- The consumer module MUST use `sc_agent.sitr` only inside the sitr section.
- The sitr section MUST stay separate from the decision call.
- The public API MUST stay documented in `sc_agent/__init__.pyi`.
- The public API MUST stay documented in the wheel README.
- The wheel README MUST give the consumer template's render command.
- Every decision call in the consumer module MUST pass the session tags.
- The consumer module MUST contain the logging wiring.
- The consumer module MUST cancel its running Run on `KeyboardInterrupt`.
- The sitr section's extraction output MUST reach the consumer's prompt.
- The wheel README MUST name the consumer template.
- A dry run MUST check that its input and arguments compose into a valid request.
- A consumer's decision call with `haiku` MUST complete when the TypeSafe key is absent.
- A consumer's decision call with `luna` MUST complete when the TypeSafe key is absent.
- The consent gate MUST ask for consent only when the Run's provider is TypeSafe.

**MUST NOT statements:**

- The consumer module MUST NOT contain consumer logic.
- The consumer module MUST NOT set a provider.
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
- [ ] A loopback unit test of each rendered module returns the mapped consumer code for an error's category.
- [ ] A loopback test runs each rendered module against the wheel built from the same commit and gets a success response with one session-log record per query.
- [ ] Scanning both rendered modules finds only wheel references to the permitted API: `send`, `send_batch`, `digest`, `logger`, `log`, `flush`, `Cancel`, and `sc_agent.sitr`.
- [ ] A loopback unit test passes `haiku` to each rendered module and finds `haiku` as the model selector of its decision call.
- [ ] A unit test finds the consumer template in the built wheel's package data.
- [ ] A CLI run of `pip download sc-decidr==<release> --no-deps` fetches the released wheel from PyPI.
- [ ] A unit test renders with `sitr` false and finds no `sc_agent.sitr` call.
- [ ] A unit test scans the `sitr` true render and finds no decision call inside the sitr section.
- [ ] Inspection of `crates/sc-agent-py/python/sc_agent/__init__.pyi` finds every public-API name.
- [ ] Inspection of `crates/sc-agent-py/README.md` finds every public-API name.
- [ ] Inspection of `crates/sc-agent-py/README.md` finds the consumer template's render command.
- [ ] A loopback test finds each query's session-log record carrying the session tags its call was given.
- [ ] A loopback unit test finds the call's events in the injected Logger.
- [ ] A loopback test raises `KeyboardInterrupt` during a batch and finds the Run cancelled.
- [ ] An offline unit test renders with `sitr` true and finds extraction output from a checked-in source file in a dry-run request's prompt.
- [ ] Inspection of `crates/sc-agent-py/README.md` finds the consumer template's path.
- [ ] A unit test gives a dry run a malformed input and finds it reported invalid.
- [ ] A CLI run of each consumer with the TypeSafe key unset and `--model haiku` completes.
- [ ] A CLI run of each consumer with the TypeSafe key unset and `--model luna` completes.
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
