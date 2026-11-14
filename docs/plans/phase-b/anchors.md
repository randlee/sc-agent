# Anchors - phase-b

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-b/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | proposed | Working client precedes template | 2026-11-14 |
| ANC-002 | proposed | Plan-anchor functional parity | 2026-11-14 |
| ANC-003 | proposed | Judge-jev sitr sequence | 2026-11-14 |
| ANC-004 | proposed | Reuse existing tests for live runs | 2026-11-14 |
| ANC-005 | proposed | Reports committed and pushed | 2026-11-14 |
| ANC-006 | proposed | One platform suffices | 2026-11-14 |
| ANC-007 | proposed | Deterministic tests | 2026-11-14 |
| ANC-008 | proposed | Three parallel tracks | 2026-11-14 |
| ANC-009 | proposed | One XHTML report per tested platform | 2026-11-14 |
| ADR-SAG-0004 | locked | No crate depends on a crate above it in the layering | 2026-11-09 |
| REQ-SAG-0002 | locked | Each crate returns its own error union carrying lower errors | 2026-11-09 |
| REQ-SAG-0006 | locked | No consumer-app or Jev-mock types in shared crates | 2026-11-09 |
| REQ-SAG-0010 | locked | No sleep calls in sc-agent source | 2026-11-09 |

## ANC-001

Anchor: The sc-agent consumer template must be built from the plan-anchor shared client after that client is defined and working.

> define/build the shared client layer (on plan-anchor) first.  once the layer is defined and working, then build sc-compose template to generate it.

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Working client precedes template

Scope: Sprints that build the shared plan-anchor client or the sc-agent consumer template.

Deterministic check to pair with the development gate: Compare the completed client revision with the template source and sprint ordering.

Fixtures:
- pass: Build and test the plan-anchor shared client, then derive the sc-agent template from it.
- fail: Build the sc-agent template before implementing the shared plan-anchor client.

## ANC-002

Anchor: The plan-anchor shared-client integration must preserve all existing plan-anchor functionality.

> integration w/ current plan-anchor (so plan anchor is unchanged functionally) on plan anchor

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Plan-anchor functional parity

Scope: The plan-anchor sprint that integrates the shared client into existing plan-anchor features.

Deterministic check to pair with the development gate: Run existing plan-anchor behavior and installation tests against the integrated client.

Fixtures:
- pass: Integrate the shared client and keep existing plan-anchor commands and behavior working.
- fail: Replace plan-anchor behavior with a reduced set of commands.

## ANC-003

Anchor: The judge-jev track must use the template and preserve its current features before adding sitr support.

> judge-jev : template -> current features -> sitr support

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Judge-jev sitr sequence

Scope: Judge-jev template adoption, feature parity and sitr integration sprints.

Deterministic check to pair with the development gate: Inspect the judge-jev sprint order and run feature and sitr tests.

Fixtures:
- pass: Adopt the shared template and preserve judge-jev features before adding sitr.
- fail: Add sitr first and defer all existing judge-jev feature integration.

## ANC-004

Anchor: Manual live integration runs must use unit tests with real providers, reusing existing mocked tests where applicable.

> I expect integration tests to be build as unit tests (ideally tests that already exist using mocks).  they are simply run w/ live agents and triggered manually.

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Reuse existing tests for live runs

Scope: Sprints that implement or run live integration tests for sc-agent or its consumers.

Deterministic check to pair with the development gate: Inspect live and controlled test selections and run the existing unit suite.

Fixtures:
- pass: Run existing mocked unit tests manually against real providers for integration.
- fail: Create an unrelated live-only test suite and retain the old unit tests separately.

## ANC-005

Anchor: The integration test process must commit and push its test report.

> The test report MUST be committed/pushed as part of the integration test process.

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Reports committed and pushed

Scope: Sprints defining or executing the integration test reporting process.

Deterministic check to pair with the development gate: Inspect the integration run procedure and the resulting git commit and remote.

Fixtures:
- pass: Commit and push the generated integration report during the integration run.
- fail: Leave the integration report only on the local machine.

## ANC-006

Anchor: One successful integration test run on one supported platform must satisfy the platform requirement.

> integration testing should be set up flexably where ONE integration test run should be required (one platform) which would typically be this mac or colima.

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: One platform suffices

Scope: Sprints defining integration test platform requirements or acceptance.

Deterministic check to pair with the development gate: Run the integration procedure with exactly one supported platform result.

Fixtures:
- pass: Accept one passing macOS run as the required platform run.
- fail: Require both macOS and Windows runs before the platform requirement is satisfied.

## ANC-007

Anchor: Any phase-b test identified as flaky must be redesigned using controlled inputs.

> Flaky tests are prohibited by design. Any flaky test identified (one that relies on a race, timing, sleeps, event-loop ordering or load) must be re-designed to be deterministic by driving the logic with controlled inputs.

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Deterministic tests

Scope: Sprints that add or change tests.

Deterministic check to pair with the development gate: Review changed tests for timing, sleep and event-order dependence.

Fixtures:
- pass: Replace the timing-dependent test with controlled synchronization.
- fail: Repeat the flaky test until a timing race passes.

## ANC-008

Anchor: After the shared plan-anchor client is complete, the sc-agent, plan-anchor and judge-jev work must be planned as three parallel tracks.

> 3 parallel tracks once plan-anchor client is complete.  template -> integration tests (sc-agent)  || integration w/ current plan-anchor (so plan anchor is unchanged functionally) on plan anchor || judge-jev : template -> current features -> sitr support

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 3.

Summary: Three parallel tracks

Scope: The phase-b plan and sprints following completion of the shared plan-anchor client.

Deterministic check to pair with the development gate: Inspect the sprint graph and the lead dispatch order.

Fixtures:
- pass: After the shared client is complete, schedule sc-agent, plan-anchor and judge-jev as parallel tracks with their stated prerequisites.
- fail: Run all plan-anchor work to completion before starting either other track.

## ANC-009

Anchor: The integration test process must produce one XHTML report for each platform tested.

> There should be one xhtml report per platform tested.

Said by user, phase-b planning, 2026-11-14. Status: proposed. Version 2.

Summary: One XHTML report per tested platform

Scope: Sprints defining integration test reports and platform coverage.

Deterministic check to pair with the development gate: Inspect the report output from separate platform runs.

Fixtures:
- pass: Render one XHTML integration report for each tested platform.
- fail: Combine the results from every tested platform into a single report.

## ADR-SAG-0004

Anchor: No sc-agent crate's Cargo.toml may depend on a crate in a higher layer, where the layers from the bottom are: types; config, secrets and log; http; decision schema; decision backends (sc-decision-local over sc-decision-systemone, sc-decision-llm over sc-llm-provider); runner.

> Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only.

Said by Rand Lee, Previously approved phase-a anchor; governing record remains approved in phase-b, 2026-11-09. Status: locked. Version 2.

Summary: No crate depends on a crate above it in the layering

Scope: adds or changes a crate dependency in a Cargo.toml

Beads: anchor `agt-mud`, plan questions `agt-mud.1`, dev questions `agt-mud.2`

Fixtures:
- pass: sc-agent-http's Cargo.toml depends only on sc-agent-types, sc-agent-secrets and sc-agent-log, all lower layers; the only crate that depends on sc-decision-runner is decidr, which sits above the runner.
- fail: sc-agent-types adds sc-decision-schema to its [dependencies] so it can name DecisionRequest.

## REQ-SAG-0002

Anchor: Every error returned by a public sc-agent function must carry the id and message of each lower-layer error that caused it.

> An error MUST carry the error id and message of every lower-layer error that caused it.

Said by Rand Lee, Previously approved phase-a anchor; governing record remains approved in phase-b, 2026-11-09. Status: locked. Version 2.

Summary: Each crate returns its own error union carrying lower errors

Scope: adds or changes an error type or a public function's return type

Beads: anchor `agt-t70`, plan questions `agt-t70.1`, dev questions `agt-t70.2`

Fixtures:
- pass: HttpError::Network wraps the ureq error and a test asserts the outer error's chain carries the ureq error's id and message.
- fail: The backend maps every lower-layer failure to a new error with the message "request failed", dropping the original error.

## REQ-SAG-0006

Anchor: No type, field or constant in the sc-agent-types or sc-decision-schema crates must be specific to plan-anchor, judge-jev or another application built on sc-agent.

> `sc-agent-types` and `sc-decision-schema` MUST NOT contain types specific to plan-anchor, judge-jev or any other consumer application.

Said by Rand Lee, Previously approved phase-a anchor; governing record remains approved in phase-b, 2026-11-09. Status: locked. Version 2.

Summary: No consumer-app or Jev-mock types in shared crates

Scope: adds or changes a type in sc-agent-types or sc-decision-schema

Beads: anchor `agt-qyd`, plan questions `agt-qyd.1`, dev questions `agt-qyd.2`

Fixtures:
- pass: Add the TypeSafe API request and response types to sc-decision-schema; no type, field or constant names plan-anchor or judge-jev.
- fail: sc-agent-types adds a PlanAnchorVerdict enum for the plan-anchor gate.

## REQ-SAG-0010

Anchor: sc-agent source code must not call std::thread::sleep, tokio::time::sleep or any other sleep function.

> sc-agent source code MUST NOT call `std::thread::sleep`, `std::thread::sleep_until`, `tokio::time::sleep` or any other sleep function.

Said by Rand Lee, Previously approved phase-a anchor; governing record remains approved in phase-b, 2026-11-09. Status: locked. Version 2.

Summary: No sleep calls in sc-agent source

Scope: adds or changes Rust source in an sc-agent crate

Beads: anchor `agt-6dm`, plan questions `agt-6dm.1`, dev questions `agt-6dm.2`

Fixtures:
- pass: The sc-agent runner waits out holdoffs with CancelToken::wait_until and a clippy rule forbids sleep calls.
- fail: The sc-agent retry loop calls std::thread::sleep(Duration::from_secs(1)) between attempts.
