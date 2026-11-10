<!-- Generated from bead agt-4p2 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Phase A: low-level sc-agent libraries

- phase: `a`
- integration branch: `integrate/phase-a`
- intent: `docs/plans/phase-a/intent.md`
- anchors: `docs/plans/phase-a/anchors.md`
- records: `docs/**` (REQ/NFR/ADR)

## Description

Define and build the low-level crates: sc-agent-types, sc-decision-schema (decision trait) and sc-treesitter. Intent: docs/plans/phase-a/intent.md. Anchors: docs/plans/phase-a/anchors.md.

## Sprints

| Sprint | Doc | Branch | Depends on | Status |
|---|---|---|---|---|
| a-1 | [Sprint A-1: sc-agent-types](sprint-a-1-sc-agent-types.md) | `sprint/a-1-sc-agent-types` | — | open |
| a-2 | [Sprint A-2: sc-decision-schema](sprint-a-2-sc-decision-schema.md) | `sprint/a-2-sc-decision-schema` | agt-4p2.5 | open |
| a-3 | [Sprint A-3: sc-treesitter](sprint-a-3-sc-treesitter.md) | `sprint/a-3-sc-treesitter` | — | open |
| a-4 | [Sprint A-4: sc-agent-config and sc-agent-secrets](sprint-a-4-config-secrets.md) | `sprint/a-4-config-secrets` | agt-4p2.1 | open |
| a-5 | [Sprint A-5: sc-agent-http](sprint-a-5-sc-agent-http.md) | `sprint/a-5-sc-agent-http` | agt-4p2.8, agt-4p2.4 | open |
| a-6 | [Sprint A-6: sc-decision-systemone and sc-decision-local](sprint-a-6-decision-backends.md) | `sprint/a-6-decision-backends` | agt-4p2.2 | open |
| a-7 | [Sprint A-7: sc-llm-provider and sc-decision-llm](sprint-a-7-llm-provider.md) | `sprint/a-7-llm-provider` | agt-4p2.2 | open |
| a-8 | [Sprint A-8: sc-agent-log](sprint-a-8-sc-agent-log.md) | `sprint/a-8-sc-agent-log` | agt-4p2.1 | open |
| a-9 | [Sprint A-9: sc-decision-runner](sprint-a-9-decision-runner.md) | `sprint/a-9-decision-runner` | agt-4p2.2 | open |
| a-10 | [Sprint A-10: decide and tsq CLIs, sc-agent Python API](sprint-a-10-clis-python.md) | `sprint/a-10-clis-python` | agt-4p2.7, agt-4p2.3, agt-4p2.6, agt-4p2.9 | open |
