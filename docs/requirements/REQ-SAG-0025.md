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

Definitions: an audit database is `<log_root>/sessions/<repo-id>/audit.sqlite`, with log root and repository identity resolved by the existing native owners. A schema owner is sc-agent core or one tool. A supported additive minor migration adds tables, indexes, views or nullable/defaulted columns without changing existing data contracts. A database upgrade changes schema; a tool upgrade changes the code that supports that schema.

- Each repository MUST have one audit database. Rust MUST own its writes, with WAL, foreign keys and transactions that commit related records together. Normal runtime transactions MUST NOT span provider calls, consumer callbacks or report rendering. Audit rows MUST remain immutable outside explicit schema migrations.
- The database MUST preserve the phase-j/k Run, session, answer, generic batch and linked walk identities and values. Runs and sessions MUST link many-to-many, including a distinct link for each cached call; core records MUST NOT acquire tool-specific item fields. Tool extensions MUST reference core identities and MUST NOT duplicate raw queries or answers. Cache identity MUST retain the exact request digest and resolved fixed model id.
- JSONL MUST remain dual-written in this phase. A failure after one sink commits MUST be exposed with its original cause; the system MUST NOT claim atomicity across SQLite and JSONL or resend a provider request to repair storage. Current-format import MUST report unsupported formats without adding historical schema converters.
- Ordinary opens MUST check core and invoking-tool versions without silently upgrading an existing database. A new database MAY initialize current supplied schemas. Supported newer additive minors MUST continue using known columns. A required database upgrade or an unsupported schema MUST return a typed error naming the relevant versions and next action.
- Database upgrades MUST be explicit and apply each migration and its version record transactionally. Agent guidance MUST ask the user whether to upgrade the current repository database or all known audit databases on the computer before mutation. An unsupported database schema MUST direct the agent to prepare a tool/dependency upgrade PR for user review; it MUST NOT silently downgrade, reset or delete the database. Additive core changes MAY use minor versions; destructive changes require explicit major-version migration support.
- Reports MUST read the audit database read-only, render HTML through sc-compose and provide a JSON sidecar with the same report model. Retiring a tool result store MUST preserve its commands through database-backed readers; source inputs and working files stay owned by the tool. Each sprint moving data MUST update affected skills, references, agent prompts and reports in both consumers, checking each reference to the old location.

### Rationale

One native store and shared interfaces let tools retain consistent audit data and useful reports without duplicating writers or silently changing a user's database.

### Success Criteria

**Acceptance Criteria:**

- [ ] Real temporary-database and installed-wheel tests preserve grouped writes, cached session links, typed answers and incomplete runs; controlled rollback and lock contention cases use no sleeps or timing assertions.
- [ ] Current, supported older, compatible newer minor and unsupported schema cases take the documented paths; refused opens leave schema/data unchanged and failed migrations leave their version unchanged.
- [ ] User-selected current-repository and computer-wide upgrade scopes target only the selected audit databases and report per-database outcomes.
- [ ] Both tools' reports render HTML and matching JSON from persisted database data; their installed skills explain upgrade consent, scope and the unsupported-schema PR path.
- [ ] JSONL remains enabled, partial sink failure is visible, and incompatible historical input is reported without conversion.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0016
- REQ-SAG-0018
- REQ-SAG-0022
- REQ-SAG-0023
- REQ-SAG-0024
- REQ-SAG-0703
- NFR-SAG-0004
- NFR-SAG-0005
- ADR-SAG-0019
- ADR-SAG-0021
