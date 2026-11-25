# Requirement: Per-repository SQLite audit storage

**Status:** Draft
**Version:** 0.1.0
**Created:** 2026-11-24
**Last Updated:** 2026-11-24
**Owner:** Rand Lee

---

## REQ-SAG-0025: Per-repository SQLite audit storage

**Status:** Draft

### Requirement Statement

- Each repository MUST have one SQLite audit database.
- The audit database MUST use WAL mode.
- Audit records MUST remain immutable outside schema maintenance.
- Core audit tables MUST have typed columns.
- Tool-extension audit tables MUST have typed columns.
- Audit tables MUST provide JSON columns for extension data.
- A tool MUST be able to add extension tables after release.
- The core schema MUST use major versions.
- A tool-extension schema MUST use minor versions on its core schema's major version.
- Tools MUST use the shared schema-version toolset.
- judge-jev and plan-anchor MUST store their results in the audit database.
- Reports MUST read the audit database.
- Reports MUST render HTML using the existing sc-compose report pattern.
- The general log MUST remain JSONL.

### Rationale

One repository store supports consistent audit queries and reports across tools (D25–D30).

### Success Criteria

**Acceptance Criteria:**

- [ ] Each repository has one WAL-mode audit database whose audit rows resist runtime mutation.
- [ ] Core and tool-extension tables have typed columns and JSON extension columns; tool extensions can be added after release.
- [ ] Core-major and tool-minor versions follow the shared schema toolset.
- [ ] Both tools store their results in the database and render HTML reports from it through sc-compose.
- [ ] General logs remain JSONL.

### Dependencies

**Related:**

- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
