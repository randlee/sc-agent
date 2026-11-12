# Requirement: Release preflight in CI

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-11  
**Owner:** Rand Lee  

---

## REQ-SAG-0132: Release preflight workflow and consumer checks

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- The installed `release-preflight` workflow MUST run on every release candidate and fail closed when credentials or consumer checks fail.
- Consumer-owned checks declared in `release/publish-preflight.toml` (including optional **docli** CLI HTML drift for **decidr** and **sitr**) MUST run through `.github/scripts/run-consumer-preflight.sh` without hand-editing kit workflow YAML for repo-only gates.

### Rationale

Preflight is the sole authority on whether a tag may ship; consumer extensions must use the kit seam, not forked workflows.

### Success Criteria

**Acceptance Criteria:**

- [ ] `release/publish-preflight.toml` includes the docli freshness check when customer-facing CLIs ship.
- [ ] `bash .github/scripts/release-preflight-docli.sh` exits 0 on the committed reference HTML under `site/cli/`.

### Dependencies

**Related:**

- REQ-SAG-0130
- REQ-SAG-0131
- ADR-SAG-0018
