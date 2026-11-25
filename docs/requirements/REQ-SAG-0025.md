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

Each repository MUST have one append-only SQLite audit database in WAL mode, with typed core and tool-extension tables, JSON extension columns, and a shared schema-version toolset. judge-jev and plan-anchor MUST store their results there and render their HTML reports from that database using the existing sc-compose pattern; the general log stays JSONL.

### Rationale

One repository store supports consistent audit queries and reports across tools (D25–D30).

### Dependencies

**Related:**

- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
