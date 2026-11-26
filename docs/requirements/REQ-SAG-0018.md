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
- The sc-decidr wheel is the Python wheel that ships the `sc_agent` package.
- Prompt authoring is building the text sent to the model. Result processing is handling the returned results and decision trees. Consumer logic is prompt authoring together with result processing.
- A decision call is a Run's `send` or `send_batch`.
- The consent gate is the consumer module's `allow_remote_judge` check before a decision call.

**MUST statements:**

- Every consumer MUST make its decision calls from code in its consumer module.
- The consumer module MUST pass the model selector through unchanged.
- The sc-decidr wheel's package data MUST include the consumer template.
- The consumer module MUST cancel its running Run on `KeyboardInterrupt`.
- The consent gate MUST ask for consent only when the Run's provider is TypeSafe.

**MUST NOT statements:**

- The consumer module MUST NOT contain consumer logic.

### Rationale

The owner requires every Python consumer to integrate identically, for errors, API keys, model selection, logging and session logging, and to differ only in prompt authoring and in processing results and decision trees. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context); one shipped template removes that divergence. Shipping the template in the wheel makes its version the installed wheel's version.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test scans plan-anchor and judge-jev and finds every decision call inside a module rendered from the consumer template.
- [ ] A loopback unit test passes `haiku` to each rendered module and finds `haiku` as the model selector of its decision call.
- [ ] A unit test finds the consumer template in the built wheel's package data.
- [ ] A loopback test raises `KeyboardInterrupt` during a batch and finds the Run cancelled.
- [ ] A loopback test finds consent asked for a TypeSafe Run and not asked for a `haiku` Run.
- [ ] Inspection of `sc_agent/templates/consumer.py.j2` finds no prompt text or result handling.

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
