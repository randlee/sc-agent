# b-13: Replace custom sc-lint setup with the kit action and consumer profile

Bead: `agt-b-13` · branch `sprint/b-13-sc-lint-kit` · target `integrate/phase-b`

Requirements: REQ-SAG-0132, REQ-SAG-0130, REQ-SAG-0133, REQ-SAG-0002, REQ-SAG-0131  
ADRs: ADR-SAG-0004, ADR-SAG-0018, ADR-SAG-0019, ADR-SAG-0017

## Description

## Goal
Use the sc-lint kit action and configure the requested consumer checks, removing the hand-written installer.

## Deliverables
1. Replace custom installation in CI with uses: randlee/sc-lint@v1, version from sc-lint.toml minimum_version (currently 0.8.9), and run lint ci --consumer. Remove .github/actions/setup-sc-lint/, setup_sc_lint_source.py, verify_sc_lint_checksum.py, test_setup_sc_lint_{release,checksum,source}.py, the Test sc-lint release setup CI step and their ci_scope.py entries. Preserve the runtime panic rule in the consumer profile.
2. Configure the consumer lint profile for boundary and portability plus the existing runtime panic rule. Put boundary configuration under boundaries/<crate>/ using only ADR-SAG-0004 and the actual Cargo dependency graph; do not invent boundaries. Leave historical phase-a plan reports alone.
3. Remove sc_lint_source_revision from the sc-publish consumer input and regenerate release/publish-artifacts.toml.j2 only through the pinned installer. Files owned by the sc-publish kit (install.py, README.sc-publish.md, release templates, test_install/test_release_artifacts/test_npm_release) change only through its pinned installer. If the kit still carries the source-install path, file it against sc-publish instead of hand-editing kit-owned files; retain the current generated kit output until its installer can remove that path, and report the unresolved state.
4. Own the release/ half of the parallel recovery sprint: update the prerelease post_install and verify paths to scripts/install_wheels.py and remove the live-provider preflight registration using the consumer input and pinned installer. Keep unrelated prerelease and docli preflight behavior; b-4 lands first, then b-13, so the helper exists when the release entries change.

CI changes are authorized for this sprint only. The action and profile adoption are owner-directed; no dedicated REQ currently governs them.

## Does Not Close
The remaining 0.2 sprints are planned against the post-recovery baseline.

Coordinate prerelease documentation with b-4, which owns docs/integration/prerelease.md; ensure its preflight and sc-lint setup instructions reflect the combined result.


## Design

The two wave-1 sprints have disjoint file fences. This sprint alone edits release/, including the recovery sprint's manifest wiring; the recovery sprint alone owns the relocated helper and removal of its old integration code. Their combined result preserves /prerelease --install and --publish and removes the evidence preflight. Do not edit generated kit-owned files directly; report an upstream kit defect if the pinned installer cannot produce the required change. The named sc-lint action is the source of installation; use existing CI job rather than adding a job.

## Acceptance criteria

## Acceptance Criteria
- [ ] boundary:sc-lint — the official action installs version 0.8.9 and CI executes lint ci --consumer; no custom setup, source/checksum verification scripts or their tests/ci_scope entries remain.
- [ ] adr:ADR-SAG-0004 — the consumer profile runs boundary, portability and the existing runtime panic rule; crate boundaries reflect ADR-SAG-0004 and Cargo dependencies only.
- [ ] req:REQ-SAG-0132 — the sc-publish input and generated artifacts remove sc_lint_source_revision and the live-provider preflight, point install/verify at the relocated helper, and preserve unrelated release checks. Changes to kit-owned files come from its pinned installer. Report any upstream kit blocker rather than editing it by hand.
- [ ] boundary:sc-lint — historical phase-a reports remain untouched.

- [ ] req:REQ-SAG-0002 — the consumer profile retains SCB-RUNTIME-008 and rejects production panic constructs.

## Validation
Run the repository's exact CI lint commands and the relevant existing installer and release tests with controlled inputs and no credentials or keychain. Inspect generated changes for unrelated drift; no polling or unattended live calls.

## Notes

Fence exception (ai-lead 2026-11-14): .github/scripts/tests/test_install_wheels.py is owned by agt-b-4, so b-13 does not edit it. b-4 lands first and b-13 rebases onto it, which removes any overlap in time.
Lead ruling (ai-lead 2026-11-14): b-13 owns everything in release/ (registering and repointing post_install/verify to scripts/install_wheels.py, removing the preflight) plus combined wiring verification after b-4 lands.

