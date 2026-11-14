# Requirement: Live prerelease provider integration and publication gate

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0133: Live prerelease provider integration and publication gate

**Status:** Approved

### Requirement Statement

The integration formula MUST run installed checksum-verified tagged prerelease binaries and wheels against real TypeSafe models and noul decisions, Anthropic structured-output decisions, and invalid environment-key authentication/redaction for both providers. Each tested platform MUST produce a committed and pushed well-formed XHTML report rendered through sc-compose from the owner-selected source. Supported transports MUST include local macOS, Colima Linux, and configurable Windows SSH. Publication preflight MUST require at least one complete passing platform run tied to candidate source and verified prerelease artifacts. SKIP, FAIL, omitted cases, malformed records, stale source and mismatched artifacts MUST fail closed; subsequent report-only commits MUST remain acceptable.

### Rationale

The owner directed this behavior in the Phase B plan; prerelease evidence must prove the installed product works.

### Success Criteria

**Acceptance Criteria:**

- [ ] One complete live platform run passes every required binary/wheel case, and its report is committed and pushed. Controlled tests prove missing, skipped, failed, malformed, stale and mismatched evidence is refused and report-only descendants pass.

### Dependencies

**Related:**

- REQ-SAG-0132
- ADR-SAG-0018
