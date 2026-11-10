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
| sc-decision-systemone, sc-decision-local (over sc-decision-systemone) | decision backends | SystemOneBackend, LocalBackend (+with_log, discover, wait_until_loaded) | systemone: types, secrets, http, schema; local: also config, log and sc-decision-systemone | a-6 |
| sc-llm-provider, sc-decision-llm | decision backends (llm over provider) | LlmClient, LlmDecisionBackend (+with_log), catalog_model_id | types, secrets, log, http (+schema for sc-decision-llm) | a-7 |
| sc-decision-runner | runner | DecisionRunner, RunnerConfig, RunnerError | types, config, log, http, schema | a-9 |
| decide, sc-agent-py | top (decide library = composition root) | decide library API, decide CLI, Python module | all of the above | a-10 |
| sc-treesitter (tsq bin behind the non-default `cli` feature), sc-treesitter-py | stands alone (ADR-SAG-0005) | tsq CLI, Python query API | none (workspace root only) | a-3 |

## Wave table

Every edge is `must_follow` with trigger **dev-push**: the child starts when the parent has pushed the named artifact (after its unchanged copy commit) and merges forward before each round; it never waits for the parent's QA or PR merge. Each sprint's front matter names the artifact.

| Wave | Sprints | Starts on |
|---|---|---|
| 1 | a-1 | — |
| 2 | a-3, a-4, a-8 | a-1 push of the workspace root + its types |
| 3 | a-5 | a-4 ApiKey, a-8 GeneralLogger/Level |
| 4 | a-2 | a-5 src/request.rs + src/error.rs (pushed first after a-5's copy commit) |
| 5 | a-6, a-7, a-9 | a-2's contract plus the a-4, a-5 and a-8 artifacts each names (edge table below) |
| 6 | a-10 | a-6, a-7, a-9 named types |

Sprint count 10; critical path 6 waves (a-1 → a-4/a-8 → a-5 → a-2 → a-9 → a-10); width 3 (waves 2 and 5); a-3 is its own track after wave 1.
Why more than three waves: ADR-SAG-0004 fixes the layer order, and the decision contract (a-2) carries the http types (HttpRequest/HttpResponse/HttpError, a-5), which carry ApiKey (a-4) and the logger (a-8). Hoisting the http types into a-1 would put them below their layer, so it is rejected. Dev-push triggers keep the elapsed cost of the extra waves to the time it takes each parent to push its named artifact.
No sprint split: a-4, a-6, a-7, a-3 and a-10 each own two sibling crates with a recorded vertical_rationale; splitting would add sprints without shortening the critical path.
owned_paths are disjoint across all sprints; the workspace root Cargo.toml and clippy.toml are owned by a-1 only (members = ["crates/*"], so no other sprint edits the root).

## Every must_follow edge

This table is authoritative for start conditions; the Sprints table's column lists the same sprints. A sprint starts when every artifact it lists has been pushed (dev-push).

| Sprint | must_follow | Artifact (dev-push) |
|---|---|---|
| a-2 | a-5 | sc_agent_http::{HttpRequest, HttpResponse, HttpError, LogContext (re-export of sc-agent-log's)} (crates/sc-agent-http/src/request.rs + src/error.rs + lib.rs re-export) |
| a-3 | a-1 | workspace root Cargo.toml (members = ["crates/*"], [workspace.package], [workspace.lints]) and clippy.toml (test allowances, disallowed-methods sleep rule) |
| a-4 | a-1 | sc_agent_types::{Provider, ModelId, ShortName, ReleaseDate, Catalog, ModelChoice, VarName, TypesError, ErrorId, ErrorDetail} + workspace root Cargo.toml + clippy.toml |
| a-5 | a-4 | sc_agent_secrets::ApiKey (crates/sc-agent-secrets) |
| a-5 | a-8 | sc_agent_log::{GeneralLogger, Level, LogContext} (crates/sc-agent-log) |
| a-6 | a-2 | sc_decision_schema::{Backend, DecisionRequest (private fields, DecisionRequest::new and accessors), ImagePart, SystemOneRequest, SystemOneResponse, ModelMetadataList, DecisionError} (crates/sc-decision-schema/src/) |
| a-6 | a-5 | sc_agent_http::{HttpRequest, HttpResponse, HttpError, Endpoint, Transport, Timeouts::probe, HttpRequest::with_secret, HttpRequest::with_deadline} and sc_agent_http::testing::ScriptedTransport (feature `testing`) |
| a-6 | a-4 | sc_agent_config::{RunnerKind (closed enum), RunnerHost} and sc_agent_secrets::ApiKey |
| a-6 | a-8 | sc_agent_log::{GeneralLogger, Level, LogContext} and sc_agent_log::testing::MemoryLog (feature `testing`) |
| a-7 | a-2 | sc_decision_schema::{Backend, DecisionRequest (private fields, DecisionRequest::new and accessors), ImagePart, SystemOneRequest, SystemOneResponse, Answer, DecisionError} (crates/sc-decision-schema/src/) |
| a-7 | a-5 | sc_agent_http::{HttpRequest, HttpResponse, HttpError} (src/request.rs, src/error.rs), plus Endpoint, SecretHeader, ProviderReply, Timeouts::probe, HttpClient::{may_start, send_once}, SendPermit and sc_agent_http::testing::ScriptedTransport (feature `testing`) |
| a-7 | a-4 | sc_agent_secrets::ApiKey |
| a-7 | a-8 | sc_agent_log::{GeneralLogger, Level, LogContext} and sc_agent_log::testing::MemoryLog (feature `testing`) |
| a-8 | a-1 | sc_agent_types::{SessionId, ErrorId, ErrorDetail} + workspace root Cargo.toml and clippy.toml |
| a-9 | a-2 | sc_decision_schema::{Decision, Backend, DecisionRequest, DecisionResult, DecisionError} (DecisionRequest::new/digests) |
| a-9 | a-5 | sc_agent_http::{HttpClient (may_start, may_resend, send_once), Start, SendPermit, Attempts, Pending, Next, LogContext (re-export), ResendPolicy, BusyReason, ProviderPause} + testing::{ScriptedTransport, BlockingTransport, ManualClock} (feature `testing`) |
| a-9 | a-8 | sc_agent_log::{SessionLogger, GeneralLogger, Level, LogError} + testing::{MemoryLog, MemoryRecord} (feature `testing`) |
| a-9 | a-4 | sc_agent_config::{AgentConfig, DEFAULT_WORKERS} |
| a-10 | a-6 | sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL, models_request} and sc_decision_local::{LocalBackend (incl. with_log), DaemonKind} |
| a-10 | a-7 | sc_llm_provider::{LlmClient (incl. with_log, models_request, decode_models), LlmProvider, catalog_model_id} and sc_decision_llm::LlmDecisionBackend (incl. with_log) |
| a-10 | a-9 | sc_decision_runner::{DecisionRunner, RunnerConfig, RunnerError} |

## Requirement ownership

The owner closes the record's acceptance; contributing sprints prove their boundary part and name the owner in does_not_close.

| Record | Owner of acceptance | Sprints that contribute |
|---|---|---|
| ADR-SAG-0001 | a-10 (end to end) | a-2, a-5, a-6, a-7, a-9, a-10 |
| ADR-SAG-0002 | a-10 (end to end) | a-2, a-4, a-8, a-9, a-10 |
| ADR-SAG-0004 | a-10 (end to end); every sprint checks its own Cargo.toml | a-1, a-2, a-3, a-4, a-5, a-6, a-7, a-8, a-9, a-10 |
| ADR-SAG-0005 | a-3 | a-3 |
| NFR-SAG-0001 | a-8 (40,000-session logger test); a-9 and a-10 contribute | a-8, a-9, a-10 |
| REQ-SAG-0001 | a-10 (end to end) | a-1, a-2, a-3, a-6, a-8, a-9, a-10 |
| REQ-SAG-0002 | a-10 (end to end); every sprint checks its own crate | a-1, a-2, a-3, a-4, a-5, a-6, a-7, a-8, a-9, a-10 |
| REQ-SAG-0003 | a-10 (end to end) | a-3, a-4, a-8, a-10 |
| REQ-SAG-0004 | a-10 (end to end) | a-8, a-9, a-10 |
| REQ-SAG-0005 | a-8 (last boundary) | a-4, a-8 |
| REQ-SAG-0006 | a-2 (sc-agent-types and sc-decision-schema checked in a-1 and a-2) | a-1, a-2, a-7 |
| REQ-SAG-0007 | a-10 (end to end) | a-2, a-5, a-6, a-7, a-9, a-10 |
| REQ-SAG-0008 | a-10 (end to end) | a-5, a-6, a-7, a-8, a-9, a-10 |
| REQ-SAG-0009 | a-10 (workspace); every sprint checks its own crate | a-1, a-2, a-3, a-4, a-5, a-6, a-7, a-8, a-9, a-10 |
| REQ-SAG-0010 | a-10 (workspace); every sprint checks its own crate | a-1, a-2, a-3, a-4, a-5, a-6, a-7, a-8, a-9, a-10 |
| REQ-SAG-0011 | a-10 (end to end) | a-9, a-10 |
| REQ-SAG-0012 | a-10 (end to end) | a-5, a-9, a-10 |
| REQ-SAG-0013 | a-10 (end to end) | a-1, a-4, a-5, a-6, a-9, a-10 |
| REQ-SAG-0014 | a-10 (end to end) | a-2, a-10 |
| REQ-SAG-0015 | a-10 (end to end) | a-4, a-5, a-6, a-10 |
| REQ-SAG-0101 | a-10 (end to end) | a-1, a-10 |
| REQ-SAG-0102 | a-10 (end to end) | a-1, a-4, a-10 |
| REQ-SAG-0103 | a-10 (end to end) | a-1, a-7, a-10 |
| REQ-SAG-0104 | a-10 (end to end) | a-1, a-2, a-9, a-10 |
| REQ-SAG-0201 | a-10 (end to end) | a-2, a-6, a-7, a-9, a-10 |
| REQ-SAG-0202 | a-9 (last boundary) | a-2, a-9 |
| REQ-SAG-0301 | a-3 | a-3 |
| REQ-SAG-0302 | a-3 | a-3 |
| REQ-SAG-0303 | a-3 | a-3 |
| REQ-SAG-0401 | a-10 (end to end) | a-1, a-4, a-5, a-6, a-7, a-10 |
| REQ-SAG-0501 | a-10 (end to end) | a-10 |
| REQ-SAG-0502 | a-7 | a-7 |
| REQ-SAG-0503 | a-10 (end to end) | a-7, a-10 |
| REQ-SAG-0504 | a-10 (end to end) | a-2, a-10 |
| REQ-SAG-0601 | a-10 (end to end) | a-1, a-4, a-5, a-6, a-9, a-10 |
| REQ-SAG-0602 | a-10 (end to end) | a-5, a-9, a-10 |
| REQ-SAG-0701 | a-10 (end to end) | a-2, a-4, a-9, a-10 |
| REQ-SAG-0801 | a-10 (end to end) | a-4, a-6, a-10 |
| REQ-SAG-0901 | a-10 (end to end) | a-2, a-7, a-9, a-10 |

## Outside phase A

- REQ-SAG-0302 AC2 (csharp and Go grammars): the record dates it to the second release.
- ADR-SAG-0003 (records are Raptor files): governs the record format, not code; no sprint implements it.

## Sprints

| Sprint | Doc | Branch | Depends on (every must_follow edge) | Status |
|---|---|---|---|---|
| a-1 | [Sprint A-1: sc-agent-types](sprint-a-1-sc-agent-types.md) | `sprint/a-1-sc-agent-types` | — | open |
| a-2 | [Sprint A-2: sc-decision-schema](sprint-a-2-sc-decision-schema.md) | `sprint/a-2-sc-decision-schema` | a-5 | open |
| a-3 | [Sprint A-3: sc-treesitter, tsq and sc-treesitter-py](sprint-a-3-sc-treesitter.md) | `sprint/a-3-sc-treesitter` | a-1 | open |
| a-4 | [Sprint A-4: sc-agent-config and sc-agent-secrets](sprint-a-4-config-secrets.md) | `sprint/a-4-config-secrets` | a-1 | open |
| a-5 | [Sprint A-5: sc-agent-http](sprint-a-5-sc-agent-http.md) | `sprint/a-5-sc-agent-http` | a-4, a-8 | open |
| a-6 | [Sprint A-6: sc-decision-systemone and sc-decision-local](sprint-a-6-decision-backends.md) | `sprint/a-6-decision-backends` | a-2, a-4, a-5, a-8 | open |
| a-7 | [Sprint A-7: sc-llm-provider and sc-decision-llm](sprint-a-7-llm-provider.md) | `sprint/a-7-llm-provider` | a-2, a-4, a-5, a-8 | open |
| a-8 | [Sprint A-8: sc-agent-log](sprint-a-8-sc-agent-log.md) | `sprint/a-8-sc-agent-log` | a-1 | open |
| a-9 | [Sprint A-9: sc-decision-runner](sprint-a-9-decision-runner.md) | `sprint/a-9-decision-runner` | a-2, a-4, a-5, a-8 | open |
| a-10 | [Sprint A-10: decide CLI and sc-agent Python API](sprint-a-10-clis-python.md) | `sprint/a-10-clis-python` | a-6, a-7, a-9 | open |
