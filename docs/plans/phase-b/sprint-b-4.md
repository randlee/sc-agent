# b-4: Remove integration evidence gate and reuse wheel tests for owner-run checks

Bead: `agt-b-4` · branch `sprint/b-4-move-integration-gate` · target `integrate/phase-b`

Requirements: REQ-SAG-0017, REQ-SAG-0130, REQ-SAG-0132, REQ-SAG-0133, REQ-SAG-0401, REQ-SAG-0103, REQ-SAG-0134, REQ-SAG-0002  
ADRs: ADR-SAG-0018, ADR-SAG-0019, ADR-SAG-0017

## Description

## Goal
Remove the prerelease integration evidence gate and replace it with simple owner-run live checks, preserving the existing prerelease publisher and installer.

## Deliverables
1. [ADR-SAG-0019 decision 9, REQ-SAG-0017] Delete decidr::integration_gate, its release-only error, both sc_agent integration_cases/validate_integration_results functions and exports, their tests and the build-time integration case plumbing. Nothing from the validator is moved into another crate or script.
2. [ADR-SAG-0019 decision 9, REQ-SAG-0133] Delete scripts/integration/ and its evidence/run tests, formulas/integration.formula.toml, templates/integration-report/, committed integration evidence and report files. The parallel sc-lint sprint removes the live-provider publication-gate registration. Mark REQ-SAG-0133 superseded on the b stack; update docs/integration/prerelease.md to a short SC_AGENT_LIVE=1 procedure and remove obsolete REQ-SAG-0133 gate references from docs/integration/b-1-sprint-requirements.md. Delete rather than relocate evidence validation, reporting and gate logic.
3. [ADR-SAG-0019, REQ-SAG-0130/0132] Preserve /prerelease --publish and --install. Keep scripts/prerelease_tag.py. Move only the existing generic install_wheels.py out of scripts/integration/ to scripts/install_wheels.py; inline the few manifest values it obtained from configuration.py rather than writing a replacement framework. The parallel sc-lint sprint owns release/ and updates the post_install/verify paths and removes the live-provider preflight entry through the pinned sc-publish installer; b-4 lands first, then b-13, so the helper exists when its release entries change. Preserve all other preflight checks.
4. [ADR-SAG-0019 decision 9] Reuse crates/sc-agent-py/tests/conftest.py and test_api.py: add one explicit SC_AGENT_LIVE=1 switch so live-capable outcome assertions for models list, noul send and send_batch use the owner's real config/keys and model selectors jev, haiku and luna; skip a model when its key is unavailable. Resolve each model's actual provider without forcing one. Tests that inspect loopback.requests, fixed loopback probabilities or local log files remain loopback-only; preserve their existing assertions. Default runs remain loopback-only. Error-category checks are outside this recovery sprint.

## Does Not Close
The remaining 0.2 sprints are planned against the post-recovery baseline. REQ-SAG-0132's general consumer preflight/docli mechanism and generic /prerelease functionality remain in force.

Update scripts/check-scoop-manifest.py if it still references the removed REQ-SAG-0133 gate; test_install_wheels.py is a new test owned by b-4.


## Design

Atomic deletion: the public gate, its manifest, generator, report/evidence handling, formula and its preflight registration must disappear together so neither the catalog nor a release entry points to removed code. Preserve the prerelease tag script and all unrelated sc-publish kit workflow and generic preflight behavior. The sole relocated code is the existing wheel install/verify helper required by /prerelease --install; inline its small configuration read and have the parallel sc-lint sprint update its two manifest references through the pinned installer. The existing wheel pytest suite is the only live-check surface: only explicitly marked outcome cases switch to the owner's real config and keys on SC_AGENT_LIVE=1; all nonmarked tests and every default run retain the loopback runner and platform-keyring trap. Do not force a single provider for alternate model families or require exact loopback probabilities in a real-provider result. An absent applicable key skips that live model, not a pass. No new CI check/job/workflow. Any actual credentialed run requires lead coordination and owner attendance; no polling, no unattended keychain.

Vertical rationale: public Rust/Python deletion, release wiring and the existing prerequisite installer hook form one atomic consumer migration; splitting them would leave a broken release or dead export. The release configuration is owned by the parallel sc-lint sprint; the integrated result must restore installer and preflight behavior.

## Acceptance criteria

## Acceptance Criteria
- [ ] req:REQ-SAG-0017 — decidr::integration_gate, decidr.integration_evidence, sc_agent.integration_cases and validate_integration_results are absent; no replacement evidence validator or catalog entry is introduced.
- [ ] req:REQ-SAG-0133 — its Raptor record is marked superseded on the b stack; scripts/integration/, its tests, formulas/integration.formula.toml, templates/integration-report/, committed integration evidence/report outputs are removed. The b-13 sprint removes the live-provider preflight entry; unrelated checks remain.
- [ ] req:REQ-SAG-0130, req:REQ-SAG-0132 — /prerelease --install retains post-install and verify behavior using the relocated existing helper with minimal inlined manifest reads; /prerelease --publish retains scripts/prerelease_tag.py and its unrelated manifest/workflow plumbing. The parallel sc-lint sprint owns the generated manifests and CI changes; verify the combined result.
- [ ] adr:ADR-SAG-0019 — the owner can explicitly run existing marked pytest outcomes against the installed wheel for jev, haiku and luna using real configuration and each model's selected provider; an unavailable key skips that model. Preserve loopback-only request, fixed-answer and log assertions, and prove default/nonmarked tests never access the OS keychain or real provider. No separate live script, evidence report, publish gate, CI job or unattended live call is added.

## Validation
Run deterministic product-export absence, helper install/verify and release-manifest regression tests with controlled local artifacts and no credentials/keychain; inspect the generated sc-publish outputs for unrelated drift. Actual hand-run live checks are performed only by the owner after lead coordination; no polling.

## Notes

Lead ruling (ai-lead 2026-11-14): b-4 delivers scripts/install_wheels.py, its tests, and the gate deletion. b-13 owns everything in release/: registering and repointing the prerelease post_install/verify entries to the helper, removing the preflight, and verifying the combined wiring after b-4 lands. b-4's acceptance does not include release/ wiring.

