# Requirement: sc-publish install documentation and hygiene

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-11  
**Owner:** Rand Lee  

---

## REQ-SAG-0131: Install, pin bump, and local dry-run documentation

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- `docs/development/sc-publish.md` MUST document kit checkout, `install.py` render, pin bump, and consumer-input edit workflow.
- `.gitignore` MUST ignore `.sc-publish-kit/` and other kit-local caches documented in that guide.
- `install.py --dry-run` with the pinned kit and committed consumer input MUST exit 0 on a clean tree.

### Rationale

Every consumer repository reproduces the same install recipe; undocumented drift between CI and laptops blocks release.

### Success Criteria

**Acceptance Criteria:**

- [ ] Install doc names **decidr** and **sitr** as the published CLIs and references `release/sc-publish-pin.toml`.
- [ ] Dry-run command in the doc matches CI and exits 0 when run from the repository root.

### Dependencies

**Related:**

- REQ-SAG-0130
- ADR-SAG-0018
