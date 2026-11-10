# Requirement: No sleep()

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0010: No sleep()

**Status:** Approved  

### Requirement Statement

**MUST NOT statements:**

- sc-agent source code MUST NOT call `std::thread::sleep`, `std::thread::sleep_until`, `tokio::time::sleep` or any other sleep function.

### Rationale

A sleeping thread cannot react to cancellation or new work; waits are made on events or deadlines that cancellation can end.

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search of non-test source finds no call to a sleep function.

### Dependencies

**Related:**

- REQ-SAG-0601
- REQ-SAG-0104