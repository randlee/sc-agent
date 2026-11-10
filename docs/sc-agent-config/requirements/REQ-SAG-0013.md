# Requirement: Config bounds

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0013: Config bounds

**Status:** Approved  

### Requirement Statement

Definitions: The key path is the dotted path of a configuration key within its file, such as `runners.local.port`.

**MUST statements:**

- `workers` MUST be within 1..=64.
- `max_sends` MUST be within 1..=10.
- A local runner host MUST be a hostname or an IP address only.
- A runner `kind` MUST be one of the known kinds.
- A port MUST be within 1..=65535.
- `key_env` MUST match the environment-variable name pattern.
- Every bound in this requirement MUST be checked when configuration is loaded, and a failed check MUST name the key path in its error.

**MUST NOT statements:**

- A key MUST NOT be attached to a request to a plain-http host that is not loopback.

### Rationale

A bad value found at load time names the key to fix; found later it fails mid-batch or sends a credential in clear text.

### Success Criteria

**Acceptance Criteria:**

- [ ] Loading configuration with `workers = 0` or `workers = 65` fails with an error naming the key path.
- [ ] Loading configuration with `max_sends = 0` or `max_sends = 11` fails with an error naming the key path.
- [ ] Loading a runner whose host contains a scheme, path or port fails with an error naming the key path.
- [ ] Loading a runner with an unknown `kind` fails with an error naming the key path.
- [ ] Loading a runner with port 0 fails with an error naming the key path.
- [ ] Loading a `key_env` that is not a valid environment-variable name fails with an error naming the key path.
- [ ] Loading a runner with `key_env` set on a plain-http non-loopback host fails, and no request to such a host carries a key.

### Dependencies

**Related:**

- REQ-SAG-0801
- REQ-SAG-0401
- REQ-SAG-0002