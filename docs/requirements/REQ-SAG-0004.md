# Requirement: Session log identity: repo, agent role, project and tool

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0004: Session log identity: repo, agent role, project and tool

**Status:** Approved  

### Requirement Statement

Definitions: `repo` identifies the repository a session ran in. `project` is a caller-supplied project or phase name, a block of time related to major changes. `agent_role` is a caller-supplied name for the type of work done or the agent that did it, such as `plan-anchor`, `judge-jev` or `sanity`; it is one field (agent role and agent name are the same). A consumer's product name is, for example, `plan-anchor` or `judge-jev`. `session_id` is the session's ULID. `tool` is the name of the program that sent the query: a generated consumer client's product name, or `decidr` for the CLI. `tool_version` is that program's released version. PyO3 bindings are the Python bindings of sc-agent. A slug is lowercase text using only `a-z`, `0-9`, `.`, `_` and `-`.

**MUST statements:**

- Every session log row MUST carry `session_id`.
- Every session log row MUST carry `repo`.
- Every session log row MUST carry `project`.
- Every session log row MUST carry `agent_role`.
- Every session log row MUST carry `tool`.
- Every session log row MUST carry `tool_version`.
- The `decidr` CLI MUST set `tool` to `decidr` and `tool_version` to its own version.
- A generated consumer client MUST set `tool` to the consumer's product name and `tool_version` to the consumer's released version.
- `tool` and `tool_version` MUST be accepted from the caller through the PyO3 bindings.
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

Session logs are separated and analysed by repository, by type of work and by project or phase, and each row traces back to the tool and version that sent it, and later synced to a server where repositories from many machines and owners meet.

### Success Criteria

**Acceptance Criteria:**

- [ ] A session run in a clone of `https://github.com/randlee/sc-agent.git` writes rows whose `repo` is `github.com/randlee/sc-agent`.
- [ ] A session run outside git in folder `work` on host `m5` writes rows whose `repo` is `local/m5/work`.
- [ ] A CLI call and a PyO3 call with the same `project` and `agent_role` write rows with the same values in those fields.
- [ ] A `project` of `Phase B` is rejected with an error and no row is written.
- [ ] A request with no `agent_role` is rejected with an error and no row is written.
- [ ] Every session written by the plan-anchor generated client carries `agent_role` `plan-anchor`, and every session written by the judge-jev generated client carries `agent_role` `judge-jev`.
- [ ] A `decidr send` row carries `tool` `decidr` and the CLI's version; a row written by the judge-jev generated client carries `tool` `judge-jev` and judge-jev's released version.

### Dependencies

**Related:**

- REQ-SAG-0005
- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0021