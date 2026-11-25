# Requirement: Session log identity: repo, agent role, project and tool

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0004: Session log identity: repo, agent role, project and tool

**Status:** Approved  

### Requirement Statement

Definitions: sc-agent is the Rust core that sends decision requests and writes session logs. A session row is a row of a session log (REQ-SAG-0005). A query's identity is the repository it ran in, its project (REQ-SAG-0021), its agent role, and the name and released version of the program that sent it (ADR-SAG-0021, Session identity). A session row's identity is its `session_id`, which identifies the session, and the identity of the query it records. An identity is valid when every part is present and the project and agent role are slugs. `<host>/<owner>/<repo>` has no URL scheme and no `.git` suffix. An agent role is the agent or app making the request (ADR-SAG-0021). A generated consumer client is the Python module a consumer renders from the sc-decidr wheel's template (REQ-SAG-0022), and its product name is the client-name constant it carries, such as `plan-anchor`. `<host>` is the output of `hostname`, and `<folder-name>` is the name of the working directory. A slug is lowercase text using only `a-z`, `0-9`, `.`, `_` and `-`. To refuse a query is to return an error for it without making a model call or writing a session row.

**MUST statements:**

- Every session row MUST carry its identity.
- sc-agent MUST refuse every query whose identity is not valid.
- In a git repository, a query's repository MUST be written as `<host>/<owner>/<repo>` taken from the `origin` URL.
- Outside a git repository, a query's repository MUST be `local/<host>/<folder-name>`.
- A query's project MUST be the project that REQ-SAG-0021 resolves for it.
- The `decidr` CLI MUST accept an agent role from its user.
- The Python bindings of sc-agent MUST accept an agent role from their caller.
- A generated consumer client MUST set the agent role to the client's product name.

**MUST NOT statements:**

- A generated consumer client MUST NOT let its user set the agent role.

### Rationale

Session logs are separated and analysed by repository, by type of work and by project or phase, and each row traces back to the program and version that sent it. Who sets each part, and where a refusal is raised, is carried by ADR-SAG-0021 (Session identity) and ADR-SAG-0019 decision 14.

### Success Criteria

**Acceptance Criteria:**

- [ ] Unit test `crates/sc-agent/tests/records.rs`: a written session row carries `session_id`, repository, project, agent role, program name and program version.
- [ ] Unit test `crates/sc-agent/tests/sending.rs`: a query with a missing part or a non-slug project or agent role returns an error, makes no model call and writes no row.
- [ ] Unit test `crates/sc-agent/tests/config_and_values.rs`: origin `https://github.com/randlee/sc-agent.git` gives repository `github.com/randlee/sc-agent`.
- [ ] Unit test `crates/sc-agent/tests/config_and_values.rs`: folder `work` on host `m5` outside git gives repository `local/m5/work`.
- [ ] Unit test of project resolution: the project a row carries is the one REQ-SAG-0021 resolves.
- [ ] CLI run: `decidr send --agent-role sanity` writes a row with agent role `sanity`.
- [ ] Unit test `tests/contract/test_contract.py`: a Python bindings call with agent role `sanity` writes a row with that agent role.
- [ ] Unit test of the generated plan-anchor client: its rows carry agent role `plan-anchor`.
- [ ] Unit test `crates/sc-agent/tests/config_and_values.rs`: a `project` of `Phase B` is refused and no row is written.
- [ ] Unit test `tests/contract/test_contract.py`: `decidr send` with no agent role is refused and no row is written.
- [ ] Unit test `tests/contract/test_contract.py`: a CLI call and a Python bindings call with the same `project` and `agent_role` write rows with the same values in those fields.
- [ ] Unit test of the generated judge-jev client: its rows carry agent role `judge-jev`.
- [ ] Unit test of the generated plan-anchor client: its call signature has no agent role parameter.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0021
- ADR-SAG-0002
- ADR-SAG-0019
- ADR-SAG-0021
