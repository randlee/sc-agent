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

## Phase A.1 amendments (proposed)

Each row carries a marker that sprint r-1 flips to `(Active, r-1)`; root rows
are in [`../requirements.md`](../requirements.md#phase-a1--refinement-proposed).

| ID | Amendment |
| --- | --- |
| REQ-SAG-0101 | "One variant per id" becomes "every variant maps to exactly one id in `errors.json`; an id may have several variants" — (Proposed, r-1) |
| REQ-SAG-0103 | The a-1 frozen contract is unfrozen once by r-1 (types break) and re-frozen at r-1 merge; later changes are additive — (Proposed, r-1) |
| REQ-SAG-0120 | The `observability::v2::SessionLog` port no longer depends on `sc-observability-types`; `emit` returns the local `LogRejected`; behind the default `obs` feature — (Proposed, r-1) |
| REQ-SAG-0121 | Noop and test-util doubles stay; the `sc-observability-types` dependency is removed; module deletion still waits on upstream v2 — (Proposed, r-1) |
| NFR-SAG-0001 | `ScAgentErrorResponse` field rule: `schema_version` always `1`, `retryable` only when `true`, `details` only for producer errors (ADR-SAG-0027) — (Proposed, r-1) |
