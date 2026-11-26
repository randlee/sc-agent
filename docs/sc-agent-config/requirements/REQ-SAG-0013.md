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

Definitions: `workers` is the configuration setting for the worker count used to fan out a batch. Timeouts are time limits for operations, including individual requests. A `Duration` is a span of time represented by a time struct. A configuration setting is a named setting supplied by configuration, such as `workers`. An invalid setting value is a value that violates the applicable rules for the setting, such as `workers = 0`. A key is a secret used to authenticate a request. A positive count is a whole-number count greater than zero. Plain-http describes an endpoint using the `http` URL scheme. A loopback host is named or addressed through the requesting machine’s loopback interface.

**MUST statements:**

- The value of `workers` MUST be a positive count.
- Timeouts MUST be represented as `Duration` values.
- An invalid setting value MUST result in an error.
- An error for an invalid setting value MUST identify the configuration setting with the invalid value.

**MUST NOT statements:**

- A request to a plain-http endpoint on a non-loopback host MUST NOT carry a key.

### Rationale

Detecting an invalid configuration value at load time tells the operator which setting to fix and avoids a mid-batch failure. Refusing keys on plain-http requests to non-loopback hosts prevents credentials from being sent in clear text.

Owner ruling (2026-11-21), Rand Lee: "Use time structs for time."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] Loading `workers = 0` fails with an error that identifies `workers`.
- [ ] A request to a plain-http endpoint on a non-loopback host carries no key.

### Dependencies

**Related:**

- REQ-SAG-0015
- REQ-SAG-0801
- REQ-SAG-0401
- REQ-SAG-0002