# Requirement: First-release publish dry-run

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0803: First release dry-run exercises publish order

**Status:** Approved  

### Requirement Statement

Definitions: Manifest publish order is the order of the `publish_order` values in `release/publish-artifacts.toml`. A verification location is CI or a documented local recipe. A crates.io dry run runs `cargo publish --dry-run` for every declared publishable crates.io crate in manifest publish order at a verification location. A sc-decidr wheel build is the maturin wheel build for the **sc-decidr** PyPI package.

**MUST statements:**

- The release process MUST prove the crates.io dry run.
- The release process MUST prove the sc-decidr wheel build at a verification location.

### Rationale

Production registry uploads and version tags remain out of scope until the owner dispatches a release.

Dry-run catches manifest, workspace version, and wheel build failures before credentials are spent on a real tag.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] `scripts/sc-publish-dry-run.sh` (or equivalent documented in `docs/development/sc-publish.md`) runs publish-order dry-run and the wheel build without error on the integration branch.
- [ ] `docs/development/sc-publish.md` documents the command and links this requirement.

### Dependencies

**Related:**

- REQ-SAG-0130
- REQ-SAG-0132
