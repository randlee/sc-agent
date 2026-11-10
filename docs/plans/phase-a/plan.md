<!-- Generated from bead agt-4p2 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Phase A: low-level sc-agent libraries

- phase: `a`
- integration branch: `integrate/phase-a`
- intent: `docs/plans/phase-a/intent.md`
- anchors: `docs/plans/phase-a/anchors.md`
- records: `docs/**` (REQ/NFR/ADR)

## Description

Define and build the low-level crates: sc-agent-types, sc-decision-schema (decision trait) and sc-treesitter. Intent: docs/plans/phase-a/intent.md. Anchors: docs/plans/phase-a/anchors.md.

## Design

## Boundary map

| Crate | Layer (ADR-SAG-0004) | Contract surface other sprints consume | Allowed sc-agent dependencies | Sprint |
|---|---|---|---|---|
| sc-agent-types | types | Provider, ModelId, ShortName, ReleaseDate, Catalog, ModelChoice, SessionId, CancelToken, ErrorId, ErrorDetail; workspace root Cargo.toml + clippy.toml | none | a-1 |
| sc-agent-config | config/secrets/log | AgentConfig, DEFAULT_WORKERS | types | a-4 |
| sc-agent-secrets | config/secrets/log | ApiKey, KeySource, resolve | types | a-4 |
| sc-agent-log | config/secrets/log | GeneralLogger, SessionLogger, Level, LogError; testing::MemoryLog | types | a-8 |
| sc-agent-http | http | HttpRequest, HttpResponse, HttpError, Transport, HttpClient, Attempts::may_start, ResendPolicy; testing::{ScriptedTransport, BlockingTransport} | types, secrets, log | a-5 |
| sc-decision-schema | decision schema | Backend, Decision, DecisionRequest (+validate), DecisionResult, DecisionError, question_digests | types, http | a-2 |
| sc-decision-systemone, sc-decision-local | decision backends | SystemOneBackend, LocalBackend (+with_log, discover) | types, secrets, log, http, schema | a-6 |
| sc-llm-provider, sc-decision-llm | decision backends (llm over provider) | LlmClient, LlmDecisionBackend (+with_log), catalog_model_id | types, secrets, log, http (+schema for sc-decision-llm) | a-7 |
| sc-decision-runner | runner | DecisionRunner, RunnerConfig, RunnerError | types, config, log, http, schema | a-9 |
| decide, sc-agent-py | top (decide library = composition root) | decide library API, decide CLI, Python module | all of the above | a-10 |
| sc-treesitter, sc-treesitter-py (tsq bin) | stands alone | tsq CLI, Python query API | none (workspace root only) | a-3 |

## Wave table

Every edge is `must_follow` with trigger **dev-push**: the child starts when the parent has pushed the named artifact (after its unchanged copy commit) and merges forward before each round; it never waits for the parent's QA or PR merge. Each sprint's front matter names the artifact.

| Wave | Sprints | Starts on |
|---|---|---|
| 1 | a-1 | — |
| 2 | a-3, a-4, a-8 | a-1 push of the workspace root + its types |
| 3 | a-5 | a-4 ApiKey, a-8 GeneralLogger/Level |
| 4 | a-2 | a-5 src/request.rs + src/error.rs (pushed first after a-5's copy commit) |
| 5 | a-6, a-7, a-9 | a-2 Backend/Decision/DecisionRequest/DecisionError |
| 6 | a-10 | a-6, a-7, a-9 named types |

Sprint count 10; critical path 6 waves (a-1 → a-4/a-8 → a-5 → a-2 → a-9 → a-10); width 3 (waves 2 and 5); a-3 is its own track after wave 1.
Why more than three waves: ADR-SAG-0004 fixes the layer order, and the decision contract (a-2) carries the http types (HttpRequest/HttpResponse/HttpError, a-5), which carry ApiKey (a-4) and the logger (a-8). Hoisting the http types into a-1 would put them below their layer, so it is rejected. Dev-push triggers keep the elapsed cost of the extra waves to the time it takes each parent to push its named artifact.
No sprint split: a-4, a-6, a-7, a-3 and a-10 each own two sibling crates with a recorded vertical_rationale; splitting would add sprints without shortening the critical path.
owned_paths are disjoint across all sprints; the workspace root Cargo.toml and clippy.toml are owned by a-1 only (members = ["crates/*"], so no other sprint edits the root).

## Sprints

| Sprint | Doc | Branch | Depends on | Status |
|---|---|---|---|---|
| a-1 | [Sprint A-1: sc-agent-types](sprint-a-1-sc-agent-types.md) | `sprint/a-1-sc-agent-types` | — | open |
| a-2 | [Sprint A-2: sc-decision-schema](sprint-a-2-sc-decision-schema.md) | `sprint/a-2-sc-decision-schema` | a-5 | open |
| a-3 | [Sprint A-3: sc-treesitter, tsq and sc-treesitter-py](sprint-a-3-sc-treesitter.md) | `sprint/a-3-sc-treesitter` | a-1 | open |
| a-4 | [Sprint A-4: sc-agent-config and sc-agent-secrets](sprint-a-4-config-secrets.md) | `sprint/a-4-config-secrets` | a-1 | open |
| a-5 | [Sprint A-5: sc-agent-http](sprint-a-5-sc-agent-http.md) | `sprint/a-5-sc-agent-http` | a-8, a-4 | open |
| a-6 | [Sprint A-6: sc-decision-systemone and sc-decision-local](sprint-a-6-decision-backends.md) | `sprint/a-6-decision-backends` | a-2 | open |
| a-7 | [Sprint A-7: sc-llm-provider and sc-decision-llm](sprint-a-7-llm-provider.md) | `sprint/a-7-llm-provider` | a-2 | open |
| a-8 | [Sprint A-8: sc-agent-log](sprint-a-8-sc-agent-log.md) | `sprint/a-8-sc-agent-log` | a-1 | open |
| a-9 | [Sprint A-9: sc-decision-runner](sprint-a-9-decision-runner.md) | `sprint/a-9-decision-runner` | a-2 | open |
| a-10 | [Sprint A-10: decide CLI and sc-agent Python API](sprint-a-10-clis-python.md) | `sprint/a-10-clis-python` | a-7, a-6, a-9 | open |
