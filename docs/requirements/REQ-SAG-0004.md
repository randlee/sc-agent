# Requirement: Session log identity: repo, agent role and project

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0004: Session log identity: repo, agent role and project

**Status:** Approved  

### Requirement Statement

Definitions: `repo` identifies the repository a session ran in. `project` is a caller-supplied project or phase name, a block of time related to major changes. `agent_role` is a caller-supplied name for the type of work done or the agent that did it, such as `plan-anchor`, `judge-jev` or `sanity`; it is one field (agent role and agent name are the same). `session_id` is the session's ULID. A slug is lowercase text using only `a-z`, `0-9`, `.`, `_` and `-`.

**MUST statements:**

- Every session log row MUST carry `session_id`, `repo`, `project` and `agent_role`.
- `repo` MUST be the repository's git `origin` URL normalized to `<host>/<owner>/<repo>`, with no scheme and no `.git` suffix.
- When the working folder is not in a git repository, `repo` MUST be `local/<host>/<folder-name>`.
- `project` and `agent_role` MUST be accepted from the user through the CLI and the PyO3 bindings.
- `project` and `agent_role` MUST be slugs.

**MUST NOT statements:**

- sc-agent MUST NOT silently rewrite an invalid `project` or `agent_role`; it MUST return an error instead.

### Rationale

Session logs are separated and analysed by repository, by type of work and by project or phase, and later synced to a server where repositories from many machines and owners meet.

### Success Criteria

**Acceptance Criteria:**

- [ ] A session run in a clone of `https://github.com/randlee/sc-agent.git` writes rows whose `repo` is `github.com/randlee/sc-agent`.
- [ ] A session run outside git in folder `work` on host `m5` writes rows whose `repo` is `local/m5/work`.
- [ ] A CLI call and a PyO3 call with the same `project` and `agent_role` write rows with the same values in those fields.
- [ ] A `project` of `Phase B` is rejected with an error and no row is written.

### Dependencies

**Related:**

- REQ-SAG-0005
- ADR-SAG-0002