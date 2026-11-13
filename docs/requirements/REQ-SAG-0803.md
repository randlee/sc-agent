# Requirement: First-release publish dry-run

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-11  
**Owner:** Rand Lee  

---

## REQ-SAG-0803: First release dry-run exercises publish order

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- Sprint **a-12** MUST prove `cargo publish --dry-run` in manifest publish order for every crates.io crate and maturin wheel builds for **sc-decidr** and **sitr** PyPI packages in CI or a documented local recipe.
- Production registry uploads and version tags remain out of scope until the owner dispatches a release.

### Rationale

Dry-run catches manifest, workspace version, and wheel build failures before credentials are spent on a real tag.

### Success Criteria

**Acceptance Criteria:**

- [ ] `scripts/sc-publish-dry-run.sh` (or equivalent documented in `docs/development/sc-publish.md`) runs publish-order dry-run and both wheel builds without error on the integration branch.
- [ ] Sprint **a-12** documents the command and links this requirement.

### Dependencies

**Related:**

- REQ-SAG-0130
- REQ-SAG-0132
