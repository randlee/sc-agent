# Requirement: sc-publish consumer input and five channels

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-11  
**Owner:** Rand Lee  

---

## REQ-SAG-0130: sc-publish consumer input enables crates.io, PyPI, Homebrew, Scoop, and WinGet

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- The repository MUST pin an exact sc-publish kit revision in `release/sc-publish-pin.toml`.
- The repository MUST declare a complete `release/sc-publish-consumer-input.json` for sc-agent: every publish-ordered Rust crate, both customer-facing CLI binaries (**decidr** and **sitr**), PyPI wheels **sc-decidr** (import `sc_agent`) and **sitr**, and channels **crates.io**, **pypi**, **homebrew**, **scoop**, and **winget**.
- The workspace root `Cargo.toml` MUST list every manifest `cargo_toml` path in `[workspace].members` literally so `validate-manifest` succeeds.

### Rationale

Registry publish is manifest-driven via sc-publish; ambiguous workspace globs and missing channel declarations break preflight and release automation.

### Success Criteria

**Acceptance Criteria:**

- [ ] `release/sc-publish-pin.toml` records a full 40-character kit SHA.
- [ ] Consumer input lists fourteen publish crates (**sc-treesitter**, then **sitr**, then **decidr**), release binaries **decidr** and **sitr**, PyPI names **sc-decidr** and **sitr**, and all five channels.
- [ ] `python3 .github/scripts/release_artifacts.py validate-manifest --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml` exits 0.

### Dependencies

**Related:**

- ADR-SAG-0017
- ADR-SAG-0018
- REQ-SAG-0131
- REQ-SAG-0132
- REQ-SAG-0803
