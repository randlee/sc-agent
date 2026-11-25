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

Definitions: sc-agent is the Rust core that sends decision requests and writes session logs. A session row is a row of a session log (REQ-SAG-0005). A query's identity is the repository it ran in, its project (REQ-SAG-0021), its agent role, and the name and released version of the program that sent it (ADR-SAG-0021, Session identity). A session row's identity is its `session_id`, which identifies the session, and the identity of the query it records. The repository is written `<host>/<owner>/<repo>` from the git `origin` remote, or `local/<machine>/<folder>` outside a git repository. An identity is valid when every part is present and the project and agent role are slugs. A slug is lowercase text using only `a-z`, `0-9`, `.`, `_` and `-`. To refuse a query is to return an error for it without making a model call or writing a session row.

**MUST statements:**

- Every session row MUST carry its identity.
- sc-agent MUST refuse every query whose identity is not valid.

### Rationale

Session logs are separated and analysed by repository, by type of work and by project or phase, and each row traces back to the program and version that sent it. Who sets each part, and where a refusal is raised, is carried by ADR-SAG-0021 (Session identity) and ADR-SAG-0019 decision 14.

### Success Criteria

**Acceptance Criteria:**

- [ ] `crates/sc-agent/tests/records.rs:7`: rows carry `session_id`, `repo`, `project` and `agent_role`.
- [ ] `crates/sc-agent/tests/config_and_values.rs:280`: origin `https://github.com/randlee/sc-agent.git` gives `github.com/randlee/sc-agent`; folder `work` on host `m5` outside git gives `local/m5/work`.
- [ ] `crates/sc-agent/tests/config_and_values.rs:63` and `crates/sc-agent/tests/sending.rs:453`: a `project` or `agent_role` that is not a slug is refused and no row is written.
- [ ] `tests/contract/test_contract.py:848`: `decidr send` with no agent role is refused.
- [ ] Program name and version on every row: planned in agt-j-2 deliverable J2-2.
- [ ] Creating a Run with an empty program name or version fails with `decision.missing_tool` (category `validation`) and nothing is sent: no model call and no row; a `decidr send` row names `decidr` and its version: planned in agt-j-2 deliverable J2-4.
- [ ] A generated client's rows name the client as program and agent role: planned in agt-j-3 deliverable J3-1.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0021
- ADR-SAG-0002
- ADR-SAG-0019
- ADR-SAG-0021
