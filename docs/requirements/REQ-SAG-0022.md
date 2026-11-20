# Requirement: The generated client shares a Rust-owned Logger

**Status:** Proposed  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-19  
**Owner:** Rand Lee  

---

## REQ-SAG-0022: The generated client shares a Rust-owned Logger

**Status:** Proposed  

### Requirement Statement

Definitions: The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018). The client name is the consumer's product name, a constant in the generated client, and equals its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`. The client Logger is owned by the sc-decidr wheel and named by the client (REQ-SAG-0008). Its handle is a callable accepted by both the sc-decidr and sitr wheels. A wheel call is a call to either wheel.

**MUST statements:**

- The generated client MUST obtain exactly one client Logger per process.
- The generated client MUST inject the client Logger as the general logger into every wheel call.
- With nothing injected by the app, the client Logger MUST write to `~/.sc-agent/logs/<client name>.log.jsonl`.
- The generated client MUST pass the same sc-decidr-owned handle to every decidr and sitr call.
- The session logger MUST be Rust-owned and MUST NOT be injectable from Python (REQ-SAG-0005).

**MUST NOT statements:**

- The generated client MUST NOT have a log-root variable.
- The generated client MUST NOT make a wheel call without a general logger.

### Rationale

One Logger per client puts an app's records and all its decidr and sitr calls in one file, and default logs under `~/.sc-agent/` give the owner every log in one place for audit.

### Success Criteria

**Acceptance Criteria:**

- [ ] With nothing injected, judge-jev's generated client makes one sc-decidr call, one sitr call and one failed sitr call; all three calls' records are in `~/.sc-agent/logs/judge-jev.log.jsonl`, the session rows are in `~/.sc-agent/sessions/<repo>/judge-jev/<project>/sessions.jsonl`, and no `decidr.log.jsonl` or `sitr.log.jsonl` is written.
- [ ] With nothing injected, plan-anchor's generated client makes one sc-decidr call and its records are in `~/.sc-agent/logs/plan-anchor.log.jsonl`.
- [ ] The same Rust-owned handle is used for decidr and sitr calls, and neither wheel requires the sc-observability Python package.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0008
- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
