# Requirement: Local runner timeouts

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0015: Local runner timeouts

**Status:** Approved  

### Requirement Statement

Definitions: The request timeout bounds one request to a runner. The local runner is Laya, at the host and port from configuration.

**MUST statements:**

- The local runner MUST use the same request timeout as the remote providers: 120 seconds, or the call's `timeout_secs` (REQ-SAG-0702).

**MUST NOT statements:**

- Holdoff MUST NOT use sleep().

### Rationale

Local models are slower than remote providers and loading a model can take minutes; counting load time against the request timeout fails requests that would succeed.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "Use time structs for time."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Laya request with no `timeout_secs` uses a 120-second request timeout; with `timeout_secs` it uses that value.

### Dependencies

**Related:**

- REQ-SAG-0801
- REQ-SAG-0013
- REQ-SAG-0601
- REQ-SAG-0010