# Requirement: First-release publish dry-run

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0803: First release dry-run exercises publish order

**Status:** Approved  

### Requirement Statement

Definitions: A release rehearsal runs in continuous integration or follows a documented procedure on a local machine. It runs `cargo publish --dry-run` for every crates.io crate marked publishable, in the order specified by `release/publish-artifacts.toml`, and builds the **sc-decidr** Python Package Index wheel with maturin.

Production registry uploads and version tags remain out of scope until the owner dispatches a release.

**MUST statements:**

- The release process MUST run the release rehearsal without error.

### Rationale

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
