# Requirement: Log retention

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0003: Log retention

**Status:** Draft  

### Requirement Statement

Definitions: The general log is the application diagnostic log. A session log is the record of every model send that sc-agent makes. sc-observability is the logging library sc-agent uses.

**MUST statements:**

- The general log MUST use the rotation and retention built into sc-observability.
- Session logs MUST be kept indefinitely.

**MUST NOT statements:**

- sc-agent MUST NOT implement its own rotation or retention for the general log.
- sc-agent MUST NOT rotate, prune or delete session logs.

### Rationale

sc-observability already rotates and prunes the general log. Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data.

### Success Criteria

**Acceptance Criteria:**

- [ ] The general log rotates under the sc-observability retained-log policy, and no sc-agent crate contains rotation or pruning code.
- [ ] A session log written more than the sc-observability retention age ago is still present after a new session runs.

### Dependencies

**Related:**

- ADR-SAG-0002