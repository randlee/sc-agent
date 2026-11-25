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
- An extension point is a template variable or a named block of the consumer template.
- Prompt authoring is building the text sent to the model. Result processing is handling the returned results and decision trees. Consumer logic is prompt authoring together with result processing.
- A decision call is a Run's `send` or `send_batch`. The sitr section is the part of a consumer module that runs tree-sitter extraction.
- A covered release is an sc-agent release of version 0.2.0 or later. PyPI is the Python Package Index, pypi.org.

**MUST statements:**

- Every consumer MUST make its decision calls through a consumer module rendered from the consumer template.
- A consumer-specific value MUST reach the consumer module only through an extension point.
- An extension point MUST carry only consumer logic.
- The consumer template MUST ship in the sc-decidr wheel as package data.
- PyPI MUST hold the sc-decidr wheel of every covered release.
- The sitr section MUST stay separate from the decision call.

### Rationale

The owner requires every Python consumer to integrate identically, for errors, API keys, model selection, logging and session logging, and to differ only in prompt authoring and in processing results and decision trees. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context); one shipped template removes that divergence. Shipping the template in the wheel makes its version the installed wheel's version.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test scans plan-anchor and judge-jev and finds every decision call inside a module rendered from the consumer template.
- [ ] A unit test renders the template for plan-anchor and judge-jev and finds the two modules identical outside their extension points.
- [ ] A unit test finds that each extension point of the consumer template holds only consumer logic.
- [ ] A unit test finds the consumer template in the built wheel's package data.
- [ ] A CI run installs the released sc-decidr wheel from PyPI.
- [ ] A unit test scans the `sitr` true render and finds no decision call inside the sitr section.

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
