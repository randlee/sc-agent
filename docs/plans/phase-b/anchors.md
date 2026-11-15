# Anchors - phase-b

Constraints captured during planning. Each anchor is a testable statement written by the orchestrator and approved by the user; the user's verbatim words are kept beneath it as provenance. Every anchor is judged at the planning gate and again at the development gate. Edit `anchors.json`, not this file; this file is rendered.

Intent: `docs/plans/phase-b/intent.md`

| ID | Status | Summary | Stated |
| --- | --- | --- | --- |
| ANC-001 | locked | Working client precedes template | 2026-11-14 |
| ANC-002 | locked | Plan-anchor functional parity | 2026-11-14 |
| ANC-003 | locked | Judge-jev sitr sequence | 2026-11-14 |
| ANC-004 | locked | Reuse existing tests for live runs | 2026-11-14 |
| ANC-005 | locked | Reports committed and pushed | 2026-11-14 |
| ANC-006 | locked | One platform suffices | 2026-11-14 |
| ANC-007 | locked | Deterministic tests | 2026-11-14 |
| ANC-008 | locked | Three parallel tracks | 2026-11-14 |
| ANC-009 | locked | One XHTML report per tested platform | 2026-11-14 |
| ADR-SAG-0004 | locked | No crate depends on a crate above it in the layering | 2026-11-09 |
| REQ-SAG-0002 | locked | Each crate returns its own error union carrying lower errors | 2026-11-09 |
| REQ-SAG-0006 | locked | No consumer-app or Jev-mock types in shared crates | 2026-11-09 |
| REQ-SAG-0010 | locked | No sleep calls in sc-agent source | 2026-11-09 |
| ANC-010 | locked | Fix minor integration defects | 2026-11-14 |
| ANC-011 | proposed |  | 2026-11-14 |
| ANC-012 | proposed |  | 2026-11-14 |
| ANC-013 | proposed |  | 2026-11-14 |

## ANC-001

Anchor: In the phase-b bead dependency graph, the sprint that builds the sc-agent consumer template must depend on the plan-anchor sprint that delivers the shared client with its tests passing.

> define/build the shared client layer (on plan-anchor) first.  once the layer is defined and working, then build sc-compose template to generate it.

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 6.

Summary: Working client precedes template

Scope: Sprints that build the shared plan-anchor client or the sc-agent consumer template. The shared client is plan-anchor's Python module wrapping sc-decidr calls for errors, API keys, model selection, general logger and session log.

Deterministic check to pair with the development gate: Compare the completed client revision with the template source and sprint ordering.

Beads: anchor `agt-09m`, plan questions `agt-09m.1`, dev questions `agt-09m.2`

Fixtures:
- pass: Build and test the plan-anchor shared client, then derive the sc-agent template from it.
- fail: Build the sc-agent template before implementing the shared plan-anchor client.

## ANC-002

Anchor: After the plan-anchor sprint that integrates the shared client, plan-anchor's existing tests must pass with no test removed or weakened, except: Python 3.9 jobs are retired (3.11 floor), and live Jev calls require the sc-decidr wheel.

> integration w/ current plan-anchor (so plan anchor is unchanged functionally) on plan anchor

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 7.

Summary: Plan-anchor functional parity

Scope: The plan-anchor sprint that integrates the shared client into existing plan-anchor features. The shared client is plan-anchor's Python module wrapping sc-decidr calls for errors, API keys, model selection, general logger and session log.

Deterministic check to pair with the development gate: Run existing plan-anchor behavior and installation tests against the integrated client.

Beads: anchor `agt-z25`, plan questions `agt-z25.1`, dev questions `agt-z25.2`

Fixtures:
- pass: Integrate the shared client and keep existing plan-anchor commands and behavior working.
- fail: Replace plan-anchor behavior with a reduced set of commands.

## ANC-003

Anchor: In the phase-b bead dependency graph, judge-jev's sitr sprint must depend on its feature-parity sprint, and that sprint must depend on its template-adoption sprint.

> judge-jev : template -> current features -> sitr support

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 5.

Summary: Judge-jev sitr sequence

Scope: Judge-jev template adoption, feature parity and sitr integration sprints.

Deterministic check to pair with the development gate: Inspect the judge-jev sprint order and run feature and sitr tests.

Beads: anchor `agt-08a`, plan questions `agt-08a.1`, dev questions `agt-08a.2`

Fixtures:
- pass: Adopt the shared template and preserve judge-jev features before adding sitr.
- fail: Add sitr first and defer all existing judge-jev feature integration.

## ANC-004

Anchor: Each live integration test must be a unit test that also runs against mocked providers, reusing an existing mocked test where one exists; the live run executes that same test against real providers.

> I expect integration tests to be build as unit tests (ideally tests that already exist using mocks).  they are simply run w/ live agents and triggered manually.

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 5.

Summary: Reuse existing tests for live runs

Scope: Sprints that implement or run live integration tests for sc-agent or its consumers.

Deterministic check to pair with the development gate: Inspect live and controlled test selections and run the existing unit suite.

Beads: anchor `agt-8s2`, plan questions `agt-8s2.1`, dev questions `agt-8s2.2`

Fixtures:
- pass: Run existing mocked unit tests manually against real providers for integration.
- fail: Create an unrelated live-only test suite and retain the old unit tests separately.

## ANC-005

Anchor: An integration run must end with its test report on the remote branch the run was made from.

> The test report MUST be committed/pushed as part of the integration test process.

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 5.

Summary: Reports committed and pushed

Scope: Sprints defining or executing the integration test reporting process.

Deterministic check to pair with the development gate: Inspect the integration run procedure and the resulting git commit and remote.

Beads: anchor `agt-ate`, plan questions `agt-ate.1`, dev questions `agt-ate.2`

Fixtures:
- pass: Commit and push the generated integration report during the integration run.
- fail: Leave the integration report only on the local machine.

## ANC-006

Anchor: One passing integration run on any one platform, such as local macOS or Colima Linux, must be sufficient for the integration requirement; no second platform run is required.

> integration testing should be set up flexably where ONE integration test run should be required (one platform) which would typically be this mac or colima.

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 5.

Summary: One platform suffices

Scope: Sprints defining integration test platform requirements or acceptance.

Deterministic check to pair with the development gate: Run the integration procedure with exactly one supported platform result.

Beads: anchor `agt-2yh`, plan questions `agt-2yh.1`, dev questions `agt-2yh.2`

Fixtures:
- pass: Accept one passing macOS run as the required platform run.
- fail: Require both macOS and Windows runs before the platform requirement is satisfied.

## ANC-007

Anchor: Phase-b tests must not depend on races, wall-clock timing, sleeps, event-loop ordering or system load.

> Flaky tests are prohibited by design. Any flaky test identified (one that relies on a race, timing, sleeps, event-loop ordering or load) must be re-designed to be deterministic by driving the logic with controlled inputs.

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 6.

Summary: Deterministic tests

Scope: Sprints that add or change tests.

Deterministic check to pair with the development gate: Review changed tests for timing, sleep and event-order dependence.

Beads: anchor `agt-0qk`, plan questions `agt-0qk.1`, dev questions `agt-0qk.2`

Fixtures:
- pass: Coordinate test execution with controlled inputs and no wall-clock or scheduling assumptions.
- fail: Use sleep and event-loop scheduling order to synchronize the test.

## ANC-008

Anchor: In the phase-b bead dependency graph, the first sprint of each of the sc-agent, plan-anchor and judge-jev tracks must depend on the shared-client sprint, and no track's sprint may depend on another track's sprint except judge-jev's template-adoption sprint on the sc-agent consumer template sprint.

> 3 parallel tracks once plan-anchor client is complete.  template -> integration tests (sc-agent)  || integration w/ current plan-anchor (so plan anchor is unchanged functionally) on plan anchor || judge-jev : template -> current features -> sitr support

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 6.

Summary: Three parallel tracks

Scope: The phase-b plan and sprints following completion of the shared plan-anchor client. The shared client is plan-anchor's Python module wrapping sc-decidr calls for errors, API keys, model selection, general logger and session log.

Deterministic check to pair with the development gate: Inspect the sprint graph and the lead dispatch order.

Beads: anchor `agt-8ug`, plan questions `agt-8ug.1`, dev questions `agt-8ug.2`

Fixtures:
- pass: After the shared client is complete, schedule sc-agent, plan-anchor and judge-jev as parallel tracks with their stated prerequisites.
- fail: Run all plan-anchor work to completion before starting either other track.

## ANC-009

Anchor: The integration test process must produce one XHTML report for each platform tested.

> There should be one xhtml report per platform tested.

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 3.

Summary: One XHTML report per tested platform

Scope: Sprints defining integration test reports and platform coverage.

Deterministic check to pair with the development gate: Inspect the report output from separate platform runs.

Beads: anchor `agt-rvm`, plan questions `agt-rvm.1`, dev questions `agt-rvm.2`

Fixtures:
- pass: Render one XHTML integration report for each tested platform.
- fail: Combine the results from every tested platform into a single report.

## ADR-SAG-0004

Anchor: No sc-agent crate may depend on a crate in a higher layer. Layers from the bottom: sc-agent-types; sc-agent-config, sc-agent-secrets and sc-agent-log; sc-agent-http; sc-decision-schema; the decision backends sc-decision-systemone, sc-decision-local and sc-decision-llm (over sc-llm-provider); sc-decision-runner.

> Layers from the bottom: `sc-agent-types`; `sc-agent-config`, `sc-agent-secrets` and `sc-agent-log`; `sc-agent-http`; `sc-decision-schema`; the decision backends `sc-decision-systemone`, `sc-decision-local` and `sc-decision-llm` (over `sc-llm-provider`); `sc-decision-runner`, which handles decision models only.

Said by Rand Lee, Previously approved phase-a anchor; governing record remains approved in phase-b, 2026-11-09. Status: locked. Version 3.

Summary: No crate depends on a crate above it in the layering

Scope: adds or changes a crate dependency in a Cargo.toml

Beads: anchor `agt-mud`, plan questions `agt-mud.1`, dev questions `agt-mud.2`

Fixtures:
- pass: sc-agent-http's Cargo.toml depends only on sc-agent-types, sc-agent-secrets and sc-agent-log, all lower layers; the only crate that depends on sc-decision-runner is decidr, which sits above the runner.
- fail: sc-agent-types adds sc-decision-schema to its [dependencies] so it can name DecisionRequest.

## REQ-SAG-0002

Anchor: Every error returned by a public sc-agent function must carry, in its cause chain, the id and message of each lower-layer error that caused it.

> An error MUST carry the error id and message of every lower-layer error that caused it.

Said by Rand Lee, Previously approved phase-a anchor; governing record remains approved in phase-b, 2026-11-09. Status: locked. Version 5.

Summary: Each crate returns its own error union carrying lower errors

Scope: adds or changes an error type or a public function's return type

Beads: anchor `agt-t70`, plan questions `agt-t70.1`, dev questions `agt-t70.2`

Fixtures:
- pass: HttpError wraps the lower error with its id and message in the cause chain.
- fail: The backend discards the lower error id and message and returns only a generic error.

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

## ANC-010

Anchor: An integration sprint that finds a defect whose fix changes no REQ, ADR or anchor must fix it in that sprint, not defer it to a follow-up bead.

> If minor issues are encountered, I expect them fixed (assuming no requirements changed).

Said by user, phase-b planning, 2026-11-14. Status: locked. Version 4.

Summary: Fix minor integration defects

Scope: Sprints integrating the sc-agent client with plan-anchor or judge-jev.

Deterministic check to pair with the development gate: Inspect integration changes and defect tests against existing requirements.

Beads: anchor `agt-uw0`, plan questions `agt-uw0.1`, dev questions `agt-uw0.2`

Fixtures:
- pass: Fix minor integration defects discovered while implementing the unchanged requirements.
- fail: Leave a discovered minor integration defect unfixed even though the requirements did not change.

## ANC-011

Anchor: The sc-agent consumer template must generate the shared client as plan-anchor delivered it.

> define/build the shared client layer (on plan-anchor) first.  once the layer is defined and working, then build sc-compose template to generate it.

Said by user, Owner-approved split in phase-b planning, 2026-11-14. Status: proposed. Version 3.

Scope: Sprints building the sc-agent consumer template. The shared client is plan-anchor's Python module wrapping sc-decidr calls for errors, API keys, model selection, general logger and session log.

Beads: anchor `agt-eue`, plan questions `agt-eue.1`, dev questions `agt-eue.2`

Fixtures:
- pass: The generated module preserves the delivered plan-anchor shared client behavior.
- fail: The template generates a different client with missing shared behavior.

## ANC-012

Anchor: Live integration runs must be started manually, never by CI on a push, pull request or schedule.

> I expect integration tests to be build as unit tests (ideally tests that already exist using mocks).  they are simply run w/ live agents and triggered manually.

Said by user, Owner-approved split in phase-b planning, 2026-11-14. Status: proposed. Version 2.

Scope: Sprints defining or running live integration tests.

Beads: anchor `agt-9cc`, plan questions `agt-9cc.1`, dev questions `agt-9cc.2`

Fixtures:
- pass: The owner starts live integration tests manually; CI never starts them.
- fail: Run live provider tests on every pull request in CI.

## ANC-013

Anchor: A phase-b test found flaky must be redesigned to drive its logic with controlled inputs, never retried or given a longer timeout.

> Flaky tests are prohibited by design. Any flaky test identified (one that relies on a race, timing, sleeps, event-loop ordering or load) must be re-designed to be deterministic by driving the logic with controlled inputs.

Said by user, Owner-approved split in phase-b planning, 2026-11-14. Status: proposed. Version 2.

Scope: Sprints that find or change flaky tests.

Beads: anchor `agt-e0a`, plan questions `agt-e0a.1`, dev questions `agt-e0a.2`

Fixtures:
- pass: Redesign the flaky test to control its inputs and ordering.
- fail: Retry the flaky test and increase its timeout instead of redesigning it.
