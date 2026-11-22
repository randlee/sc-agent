# Requirement: Config bounds

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0013: Config bounds

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- `workers` and `max_sends` MUST be positive counts.
- Timeouts MUST be Durations.
- A runner `kind` MUST be one of the known kinds.
- `key_env` MUST match the environment-variable name pattern.
- A bad config value MUST be an error that says which setting.

**MUST NOT statements:**

- A key MUST NOT be attached to a request to a plain-http host that is not loopback.

### Rationale

A bad value found at load time names the key to fix; found later it fails mid-batch or sends a credential in clear text.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "Use time structs for time."

### Success Criteria

**Acceptance Criteria:**

- [ ] Loading configuration with `workers = 0` or `max_sends = 0` fails with an error that says which setting.
- [ ] Loading a runner with an unknown `kind` fails with an error that says which setting.
- [ ] Loading a `key_env` that is not a valid environment-variable name fails with an error that says which setting.
- [ ] Loading a runner with `key_env` set on a plain-http non-loopback host fails, and no request to such a host carries a key.

### Dependencies

**Related:**

- REQ-SAG-0015
- REQ-SAG-0801
- REQ-SAG-0401
- REQ-SAG-0002