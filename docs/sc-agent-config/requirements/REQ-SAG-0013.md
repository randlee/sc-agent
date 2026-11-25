# Requirement: Config bounds

**Status:** Approved  
**Version:** 0.3.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0013: Config bounds

**Status:** Approved  

### Requirement Statement

Definitions: A Duration is a time span represented by a time struct. `workers` is the worker count for a batch. A bad config value is a value invalid for its setting. A key is a secret used to authenticate a provider request.

**MUST statements:**

- `workers` MUST be at least 1.
- Timeouts MUST be Durations.
- A bad config value MUST be an error that says which setting.

**MUST NOT statements:**

- A request to a non-loopback host over plain HTTP MUST NOT carry a key.

### Rationale

A bad value found at load time names the key to fix; found later it fails mid-batch or sends a credential in clear text.

Owner ruling (2026-11-21), Rand Lee: "Use time structs for time."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that loading configuration with `workers = 0` fails with an error that says which setting.
- [ ] A unit test verifies that no request to a plain-http non-loopback host carries a key.

### Dependencies

**Related:**

- REQ-SAG-0015
- REQ-SAG-0801
- REQ-SAG-0401
- REQ-SAG-0002