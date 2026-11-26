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

`release-preflight` is the release gate installed by the sc-publish kit. The release candidate is the version proposed for release. Release credential checks verify the credentials required for that release. Repository-defined preflight checks are the consumer checks declared in `release/publish-preflight.toml`. Release-gating checks are the release credential checks and repository-defined preflight checks.

**MUST statements:**

- The installed `release-preflight` workflow MUST run for every release candidate.
- The installed `release-preflight` workflow MUST block release when any release-gating check fails.
- Every repository-defined preflight check declared in `release/publish-preflight.toml` MUST run through `.github/scripts/run-consumer-preflight.sh`.

**MUST NOT statements:**

- Repository-defined preflight checks MUST NOT be added by manually editing the workflow files that the sc-publish kit installs in `.github/workflows`.

### Rationale

Preflight is the sole authority on whether a tag may ship; consumer extensions must use the kit seam, not forked workflows.

The optional **docli** check compares regenerated command-line reference HTML for **decidr** and **sitr** with the committed HTML.

### Success Criteria

**Acceptance Criteria:**

- [ ] `release/publish-preflight.toml` includes the docli freshness check when customer-facing CLIs ship.
- [ ] `bash .github/scripts/release-preflight-docli.sh` exits 0 on the committed reference HTML under `site/cli/`.

### Dependencies

**Related:**

- REQ-SAG-0130
- REQ-SAG-0131
- ADR-SAG-0018
