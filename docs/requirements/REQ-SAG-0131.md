# Requirement: sc-publish install documentation and hygiene

**Status:** Approved  
**Version:** 0.1.1  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0131: Install, pin bump, and local dry-run documentation

**Status:** Approved  

### Requirement Statement

Definitions: The kit is sc-publish. Kit checkout fetches the revision pinned in `release/sc-publish-pin.toml` into `.sc-publish-kit/`. The `install.py` render runs the pinned kit installer against `release/sc-publish-consumer-input.json`. A pin bump changes the pinned kit revision. Consumer input is `release/sc-publish-consumer-input.json`. Kit-local caches are locally stored caches for sc-publish. A clean tree has no uncommitted changes.

**MUST statements:**

- `docs/development/sc-publish.md` MUST document kit checkout.
- `docs/development/sc-publish.md` MUST document the `install.py` render.
- `docs/development/sc-publish.md` MUST document the pin bump.
- `docs/development/sc-publish.md` MUST document the consumer-input edit workflow.
- `.gitignore` MUST ignore `.sc-publish-kit/`.
- `.gitignore` MUST ignore other kit-local caches documented in `docs/development/sc-publish.md`.
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
