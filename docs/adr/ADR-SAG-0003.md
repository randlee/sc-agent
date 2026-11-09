# Decision: Requirements and decisions are Raptor records

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0003: Requirements and decisions are Raptor records

**Status:** Draft  

### Related Documents

**External References:**

- [randlee/raptor PR 98](https://github.com/randlee/raptor/pull/98)

### Context

Requirements, non-functional requirements and architecture decisions are read by agents and by Raptor.

### Decision

Every REQ, NFR and ADR is one Markdown file written from the Raptor authoring templates (`req-new.md.j2`, `nfr-new.md.j2`, `adr-new.md.j2` on the `integration/judge-jev` branch of randlee/raptor). Repository-wide records are under `docs/requirements/`, `docs/nfr/` and `docs/adr/`; crate-specific records are under the same folders inside `docs/<crate>/`. Changes to any record are made only in plan review and approved by the owner.

### Rationale

Records that import into Raptor with no diagnostics can be queried and judged by tools, and owner approval in plan review stops requirements drifting to match code.

### Consequences

**Positive:**

- Every record imports into Raptor and can be judged by judge-jev.

**Negative:**

- Records are edited through templates, not by hand.

### Alternatives Considered

#### Alternative 1: Free-form requirement tables

**Description:**

Keep requirements as table rows in one Markdown file.

**Pros:**

- Compact.

**Cons:**

- Not parseable by Raptor.
- Rows were rewritten to match code without owner approval.

**Why Rejected:**

Records must be parseable by Raptor and changed only with owner approval.