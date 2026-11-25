# Requirement: Config bounds

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0013: Config bounds

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- `workers` MUST be a positive count.
- Timeouts MUST be Durations.
- A bad config value MUST be an error that says which setting.

**MUST NOT statements:**

- A key MUST NOT be attached to a request to a plain-http host that is not loopback.

### Rationale

A bad value found at load time names the key to fix; found later it fails mid-batch or sends a credential in clear text.

Owner ruling (2026-11-21), Rand Lee: "Use time structs for time."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] Loading configuration with `workers = 0` fails with an error that says which setting.
- [ ] No request to a plain-http non-loopback host carries a key.

### Dependencies

**Related:**

- REQ-SAG-0015
- REQ-SAG-0801
- REQ-SAG-0401
- REQ-SAG-0002