# Requirement: Local runner timeouts

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0015: Local runner timeouts

**Status:** Approved  

### Requirement Statement

Definitions: The request timeout bounds one request to a runner. A local runner is a local decision runner (REQ-SAG-0801), `laya` or `decider`, at the host and port from configuration. Holdoff is a wait a `Run` (ADR-SAG-0001) makes before sending a request.

**MUST statements:**

- A local runner MUST use the request timeout that REQ-SAG-0702 sets for the remote providers.

**MUST NOT statements:**

- Holdoff MUST NOT use sleep().

### Rationale

Local models are slower than remote providers and loading a model can take minutes; counting load time against the request timeout fails requests that would succeed.

Owner ruling (2026-11-21), Rand Lee: "Use time structs for time."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust unit test sends a Laya request and a decider request through a Run with no `timeout_secs` and checks each uses a 120-second request timeout, then through a Run with `timeout_secs` and checks each uses that value.

### Dependencies

**Related:**

- REQ-SAG-0801
- REQ-SAG-0013
- REQ-SAG-0601
- REQ-SAG-0010