# Intent - phase-b

## What this plan is for

Plan the sc-agent 0.2 decision interface and a reusable Python client that is developed and proven in plan-anchor before its sc-compose template is built. Use that common client to support plan-anchor and judge-jev without changing their existing functionality.

## What success looks like

- After the shared client works, the sc-agent, plan-anchor and judge-jev tracks can progress in parallel subject to their real prerequisites; judge-jev's sitr work follows its existing-feature integration.
- Manually triggered live integration tests reuse applicable unit tests. Each tested platform receives an XHTML-pattern report committed and pushed as part of the run; one successful platform is sufficient.
- The plan and its verification preserve the user's directives and use deterministic tests.

## What is deliberately out of scope

- An automatic live-evidence publication gate.

## Anchors

Anchors for this plan live in `anchors.json` and are rendered to `anchors.md`.
