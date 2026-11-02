# sc-agent-types — requirements

Shared types for all sc-agent crates and future launchers.

| ID | Requirement | Sprint |
| --- | --- | --- |
| REQ-SAG-0101 | `JevError` covers README error table | a-1 |
| REQ-SAG-0102 | Payload digest helpers for cross-impl fixture | a-1 |
| REQ-SAG-0103 | Frozen `JudgeBackend` / `JudgeRequest` / domain types + contract tests | a-1 |
| NFR-SAG-0001 | `ScAgentErrorResponse` for CLI stderr JSON | a-1 |
| REQ-SAG-0120 | Temporary `observability::v2::SessionLog` port (`sc-observability-types` dep) | a-1 |
| REQ-SAG-0121 | Noop + test-util doubles; remove module when upstream v2 ships | a-1 |

Domain types: `Question`, `ChoiceAnswer`, `PayloadBundle`, `BackendId`,
`JudgeOutcome`, `EqualInputRecord`.

Trait: `JudgeBackend` (see `architecture.md`).
