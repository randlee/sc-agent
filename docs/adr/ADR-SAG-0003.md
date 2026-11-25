# Decision: Requirements and decisions are Raptor records

**Status:** Approved  
**Version:** 0.1.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0003: Requirements and decisions are Raptor records

**Status:** Approved  

### Related Documents

**External References:**

- [randlee/raptor](https://github.com/randlee/raptor) authoring templates

### Context

Requirements, non-functional requirements and architecture decisions are read by agents and by Raptor.
The problem is that free-form records cannot be reliably imported into Raptor for tools to query and judge.

### Decision

Every REQ, NFR and ADR is one Markdown file written from the Raptor authoring templates (`req-new.md.j2`, `nfr-new.md.j2`, `adr-new.md.j2` in randlee/raptor). Repository-wide records are under `docs/requirements/`, `docs/nfr/` and `docs/adr/`; crate-specific records are under the same folders inside `docs/<crate>/`. Changes to any record are made only in plan review and approved by the owner.

### Rationale

Records that import into Raptor with no diagnostics can be queried and judged by tools, and owner approval in plan review keeps every change deliberate.

### Consequences

**Positive:**

- Every record imports into Raptor and can be queried.

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

**Why Rejected:**

Records must be parseable by Raptor and changed only with owner approval.
