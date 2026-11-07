# sc-agent-types — requirements

Shared types for all sc-agent crates and future launchers.

**Sprint** = the sprint that implements the requirement. The owning `req:`
acceptance root, when it differs, is shown as "root a-N"; the closure table in
[`plans/phase-a/plan.md`](../plans/phase-a/plan.md#requirement-closure-integration-req-roots) is authoritative.

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0101 | `JevError` covers README error table | a-1 (root a-7) |
| REQ-SAG-0102 | Payload digest helpers for cross-impl fixture | a-1 (root a-8) |
| REQ-SAG-0103 | Frozen `JudgeBackend` / `JudgeRequest` / domain types + contract tests | a-1 |
| REQ-SAG-0104 | `sc/subjects/1` schema types + fixture | a-1 |
| NFR-SAG-0001 | `ScAgentErrorResponse` for CLI stderr JSON | a-1 |
| REQ-SAG-0120 | Temporary `observability::v2::SessionLog` port (`sc-observability-types` dep) | a-1 |
| REQ-SAG-0121 | Noop + test-util doubles; remove module when upstream v2 ships | a-1 |

Domain types: `Question`, `ChoiceAnswer`, `PayloadBundle`, `BackendId`,
`JudgeOutcome`, `EqualInputRecord`.

Trait: `JudgeBackend` (see `architecture.md`).
