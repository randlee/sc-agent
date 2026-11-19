# Requirement: Session log identity: repo, agent role and project

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0004: Session log identity: repo, agent role and project

**Status:** Approved  

### Requirement Statement

Definitions: `repo` identifies the repository a session ran in. `project` is a caller-supplied project or phase name, a block of time related to major changes. `agent_role` is a caller-supplied name for the type of work done or the agent that did it, such as `plan-anchor`, `judge-jev` or `sanity`; it is one field (agent role and agent name are the same). A consumer's product name is, for example, `plan-anchor` or `judge-jev`. `session_id` is the session's ULID. PyO3 bindings are the Python bindings of sc-agent. A CLI (command-line interface) is an sc-agent command-line program, such as decidr or sitr. A slug is lowercase text using only `a-z`, `0-9`, `.`, `_` and `-`. An invalid `project` or `agent_role` is one that is not a slug. To resolve a setting is to choose its value from the layers that can set it. A required field is one without which a session write is rejected with an error. To return an error is to end the call with an error instead of a result. Normalizing the `origin` URL to `<host>/<owner>/<repo>` drops its scheme, user name, password and port, then one trailing `/` and one trailing `.git`.

**MUST statements:**

- Every session log row MUST include the `session_id` field.
- Every session log row MUST include the `repo` field.
- Every session log row MUST include the `project` field.
- Every session log row MUST include the `agent_role` field.
- `repo` MUST be the repository's git `origin` URL normalized to `<host>/<owner>/<repo>`.
- `repo` MUST have no scheme.
- `repo` MUST have no `.git` suffix.
- When the working folder is not in a git repository, `repo` MUST be `local/<host>/<folder-name>`.
- `project` MUST be accepted from the user through the CLI.
- `project` MUST be accepted from the user through the PyO3 bindings.
- `agent_role` MUST be accepted from the user through the CLI.
- `agent_role` MUST be accepted from the user through the PyO3 bindings.
- `project` MUST be required.
- `project` MUST be resolved as REQ-SAG-0021 states.
- `agent_role` MUST be required.
- A generated consumer client MUST hardcode `agent_role` to the consumer's product name.
- `project` MUST be a slug.
- `agent_role` MUST be a slug.
- When `project` is invalid, sc-agent MUST return an error.
- When `agent_role` is invalid, sc-agent MUST return an error.

**MUST NOT statements:**

- sc-agent MUST NOT silently rewrite an invalid `project`.
- sc-agent MUST NOT silently rewrite an invalid `agent_role`.
- A generated consumer client MUST NOT let its user set `agent_role`.

### Rationale

Session logs are separated and analysed by repository, by type of work and by project or phase, and later synced to a server where repositories from many machines and owners meet.

### Success Criteria

**Acceptance Criteria:**

- [ ] A session run in a clone of `https://github.com/randlee/sc-agent.git` writes rows whose `repo` is `github.com/randlee/sc-agent`.
- [ ] A session run outside git in folder `work` on host `m5` writes rows whose `repo` is `local/m5/work`.
- [ ] A CLI call and a PyO3 call with the same `project` and `agent_role` write rows with the same values in those fields.
- [ ] A `project` of `Phase B` is rejected with an error and no row is written.
- [ ] A request with no `agent_role` is rejected with an error and no row is written.
- [ ] Every session written by the plan-anchor generated client carries `agent_role` `plan-anchor`, and every session written by the judge-jev generated client carries `agent_role` `judge-jev`.

### Dependencies

**Related:**

- REQ-SAG-0005
- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0021