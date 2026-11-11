# Decision: First release surfaces via sc-publish

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-11  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-11  

---

## ADR-SAG-0017: First release surfaces

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0130
- REQ-SAG-0131
- REQ-SAG-0132
- REQ-SAG-0803

### Context

Phase A ships libraries, the **decidr** decision CLI, the **sitr** tree-sitter CLI, and Python wheels. Release must use one manifest-driven kit across five consumer channels.

### Decision

Phase A first release surfaces are **crates.io** (thirteen workspace crates), **PyPI** (`sc-agent` and `sc-treesitter` wheels), **Homebrew**, **Scoop**, and **WinGet** (archives containing **decidr** and **sitr**), plus GitHub Release assets from the same tag. Sprints **a-11** (kit install) and **a-12** (dry-run) close the publish requirements; production uploads wait for owner dispatch.

**Key Principles:**

- Customer-facing CLI names on channels are **decidr** and **sitr**.
- PyPI distribution names remain **sc-agent** and **sc-treesitter**.
- No ad-hoc per-channel workflow literals for crate lists or binary names.

### Consequences

**Positive:**

- One tag drives every channel from `release/publish-artifacts.toml`.

**Negative:**

- Workspace `Cargo.toml` must list members literally for kit validation.
