# Requirement: sc-publish consumer input and five channels

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0130: sc-publish consumer input enables crates.io, PyPI, Homebrew, Scoop, and WinGet

**Status:** Approved  

### Requirement Statement

Definitions: The sc-publish kit is the release tooling pinned in `release/sc-publish-pin.toml`. Its consumer input is `release/sc-publish-consumer-input.json`. A complete consumer input contains every publish-ordered Rust crate, both customer-facing CLI binaries, the PyPI wheel, and all five channels listed below. A CLI is a command-line interface; API means application programming interface. PyPI is the Python Package Index. `sc-agent-py` is the Rust crate shipped as the `sc-decidr` PyPI wheel. `decidr` and `sitr` are the customer-facing CLI binaries. A manifest `cargo_toml` path is a crate manifest path listed in `release/publish-artifacts.toml`. A publish-ordered Rust crate is a crate listed in that manifest in publish order. The four Rust products are `sc-agent`, `sc-treesitter`, `sitr` and `decidr`. Dependency order places each product after its dependencies.

**MUST statements:**

- The repository MUST pin an exact sc-publish kit revision in `release/sc-publish-pin.toml`.
- A complete `release/sc-publish-consumer-input.json` for sc-agent MUST be declared by the repository.
- The consumer input MUST list every publish-ordered Rust crate.
- The consumer input MUST list **decidr** as a customer-facing CLI binary.
- The consumer input MUST list **sitr** as a customer-facing CLI binary.
- The consumer input MUST list the PyPI wheel **sc-decidr** (import `sc_agent`, holding the decidr and sitr APIs).
- The consumer input MUST declare the **crates.io** channel.
- The consumer input MUST declare the **pypi** channel.
- The consumer input MUST declare the **homebrew** channel.
- The consumer input MUST declare the **scoop** channel.
- The consumer input MUST declare the **winget** channel.
- The Rust product `sc-agent` MUST be declared publishable on crates.io.
- The Rust product `sc-treesitter` MUST be declared publishable on crates.io.
- The Rust product `sitr` MUST be declared publishable on crates.io.
- The Rust product `decidr` MUST be declared publishable on crates.io.
- The four Rust products’ crates.io declarations MUST be in dependency order.
- `sc-agent-py` MUST remain unpublished on crates.io.
- The workspace root `Cargo.toml` MUST list all manifest `cargo_toml` paths verbatim in `[workspace].members`.

### Rationale

Registry publish is manifest-driven via sc-publish; ambiguous workspace globs and missing channel declarations break preflight and release automation. Listing paths exactly allows `validate-manifest` to succeed.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.


### Success Criteria

**Acceptance Criteria:**

- [ ] Inspect `[workspace].members` in the workspace root `Cargo.toml` for each exact manifest `cargo_toml` path.

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
