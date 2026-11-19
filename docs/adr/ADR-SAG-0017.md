# Decision: First release surfaces via sc-publish

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-14  
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

**External References:**

- [owner commit a6fca64](https://github.com/randlee/sc-agent/commit/a6fca6409d46900541c46fbb9d1fbfbff968770e) the PyPI rename; version 0.3.0 (2026-11-14, owner-approved) records fourteen published crates including decidr under owner decision `agt-ln6` (OPEN-B-007, ruling 2026-11-14T11:48:19Z).

### Context

Phase A ships libraries, the **decidr** decision CLI, the **sitr** tree-sitter CLI, and Python wheels. Release must use one manifest-driven kit across five consumer channels.

The problem is that every channel must release from the same tag, and per-channel workflow literals for crate lists or binary names would let the channels list different crates or binaries.

### Decision

Phase A first release surfaces are **crates.io** (the crates listed in `release/publish-artifacts.toml`), **PyPI** (`sc-decidr` and `sitr` wheels), **Homebrew**, **Scoop**, and **WinGet** (archives containing **decidr** and **sitr**), plus GitHub Release assets from the same tag. Sprints **a-11** (kit install) and **a-12** (dry-run) close the publish requirements; production uploads wait for owner dispatch.

**Key Principles:**

- Customer-facing CLI names on channels are **decidr** and **sitr**.
- PyPI distribution names are **sc-decidr** and **sitr**.
- No ad-hoc per-channel workflow literals for crate lists or binary names.

### Rationale

- One manifest-driven kit drives all five consumer channels from the same tag.

### Consequences

**Positive:**

- One tag drives every channel from `release/publish-artifacts.toml`.

**Negative:**

- Workspace `Cargo.toml` must list members literally for kit validation.

### Alternatives Considered

#### Alternative 1: Per-channel workflow literals

**Description:**

Each channel's workflow writes its own crate list and binary names as literals.

**Why Rejected:**

The Key Principles of this record forbid ad-hoc per-channel workflow literals for crate lists or binary names, and REQ-SAG-0132 requires consumer extensions to use the kit seam, not forked workflows.
