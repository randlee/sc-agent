# Requirement: Python 3.11 minimum

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0134: Python 3.11 minimum

**Status:** Approved

### Requirement Statement

Both Python distributions, sc-decidr and sitr, MUST require Python 3.11 or newer in their package metadata. Both MUST build, install and pass their Python tests on Python 3.11.

### Rationale

The owner directed this behavior in the Phase B plan; prerelease evidence must prove the installed product works.

### Success Criteria

**Acceptance Criteria:**

- [ ] Wheel METADATA requires >=3.11; the CI wheel jobs run their tests on Python 3.11.

### Dependencies

**Related:**

- REQ-SAG-0132
- ADR-SAG-0018
