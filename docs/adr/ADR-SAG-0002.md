# Decision: Session logs are JSONL first, then SQLite with server sync

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-10  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-09  

---

## ADR-SAG-0002: Session logs are JSONL first, then SQLite with server sync

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0001
- REQ-SAG-0003
- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0202
- NFR-SAG-0001

### Context

The owner builds tools that read session logs to tune consumer code, analysing 10,000 to 20,000 or more decisions at a time. One task produced 40,000 sessions, each a single Jev query with its own ULID and question digest.

**Problem Statement:**

Session logs must be separable by repository, type of work and project, kept indefinitely, and storable long term on a server.

### Decision

For the MVP, session logs are JSONL under `<log_root>/logs/<repo>/<agent_role>/<project>/`. Later they load into a local SQLite database that syncs to a server for long-term storage. Every row carries its own `session_id`, `repo`, `project` and `agent_role`. All sessions of one repo, agent role and project are appended to one file in that directory; a project lasts under a day to several days.

**Key Principles:**

- Every row describes itself, so it stays meaningful when moved or loaded.
- Session logs are never rotated or deleted.

### Rationale

JSONL is simple to write and to read with the owner's tools now. Self-describing rows load into SQLite without parsing paths, and SQLite gives indexed retrieval of a response by its question digest.

### Consequences

**Positive:**

- Path globs select a repository, role or project without opening files.

**Negative:**

- Until the SQLite stage, retrieving a response by digest needs a load of the JSONL rows.

### Alternatives Considered

#### Alternative 1: One JSONL file per session

**Description:**

Write each session to its own file named by its ULID.

**Pros:**

- One writer per file.

**Cons:**

- A task of 40,000 sessions creates 40,000 files.

**Why Rejected:**

Tasks create tens of thousands of single-query sessions.

#### Alternative 2: One JSONL file per run

**Description:**

Write each caller run to its own file.

**Pros:**

- Files never grow across runs.

**Cons:**

- Thousands of runs leave thousands of files in one folder.

**Why Rejected:**

Too many files; a project's sessions belong together.

#### Alternative 3: Encode repo, project and role in one hyphenated name

**Description:**

Name logs `<repo>-<project>-<agent_role>`.

**Pros:**

- Short names.

**Cons:**

- Repository names contain hyphens, so the parts cannot be recovered from the name.

**Why Rejected:**

Tools must separate by each part.