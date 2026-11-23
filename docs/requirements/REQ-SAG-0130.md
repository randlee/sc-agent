# Requirement: sc-publish consumer input and five channels

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0130: sc-publish consumer input enables crates.io, PyPI, Homebrew, Scoop, and WinGet

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- The repository MUST pin an exact sc-publish kit revision in `release/sc-publish-pin.toml`.
- The repository MUST declare a complete `release/sc-publish-consumer-input.json` for sc-agent: every publish-ordered Rust crate, both customer-facing CLI binaries (**decidr** and **sitr**), the PyPI wheel **sc-decidr** (import `sc_agent`, holding the decidr and sitr APIs), and channels **crates.io**, **pypi**, **homebrew**, **scoop**, and **winget**.
- The four Rust products (`sc-agent`, `sc-treesitter`, `sitr` and `decidr`) MUST be declared publishable on crates.io in dependency order. `sc-agent-py` MUST remain unpublished on crates.io; it ships as the PyPI wheel.
- The workspace root `Cargo.toml` MUST list every manifest `cargo_toml` path in `[workspace].members` literally so `validate-manifest` succeeds.

### Rationale

Registry publish is manifest-driven via sc-publish; ambiguous workspace globs and missing channel declarations break preflight and release automation.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.


### Success Criteria

**Acceptance Criteria:**

- [ ] `release/sc-publish-pin.toml` records a full 40-character kit SHA.
- [ ] Consumer input lists every `[[crates]]` entry of `release/publish-artifacts.toml` in that file's publish order (**sc-agent**, then **sc-treesitter**, then **sitr**, then **decidr**), release binaries **decidr** and **sitr**, PyPI name **sc-decidr**, and all five channels.
- [ ] `python3 .github/scripts/release_artifacts.py validate-manifest --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml` exits 0.

### Dependencies

**Related:**

- ADR-SAG-0017
- ADR-SAG-0018
- REQ-SAG-0131
- REQ-SAG-0132
- REQ-SAG-0803
