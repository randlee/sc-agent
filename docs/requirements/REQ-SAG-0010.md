# Requirement: No sleep()

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0010: No sleep()

**Status:** Approved  

### Requirement Statement

**MUST NOT statements:**

- sc-agent source code MUST NOT call `std::thread::sleep`, `std::thread::sleep_until`, `tokio::time::sleep` or any other sleep function.

**Notes:**

- This requirement is met by a timed wait that cancellation can end, such as a wait on a channel or condition variable. It does not require a clock abstraction or an injected time source.

### Rationale

A sleeping thread cannot react to cancellation or new work; waits are made on events or deadlines that cancellation can end.

Owner ruling (2026-11-20), Rand Lee: "No sleep (is async requirement)."

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search of non-test source finds no call to a sleep function.

### Dependencies

**Related:**

- REQ-SAG-0601
- REQ-SAG-0104