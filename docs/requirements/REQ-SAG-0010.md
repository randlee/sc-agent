# Requirement: No sleep()

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0010: No sleep()

**Status:** Approved  

### Requirement Statement

`sc-agent` is this project. A sleep function includes `std::thread::sleep`, `std::thread::sleep_until` and `tokio::time::sleep`.

**MUST NOT statements:**

- The source code of this project MUST NOT call any sleep function.

### Rationale

A sleeping thread cannot react to cancellation or new work; waits are made on events or deadlines that cancellation can end. This requirement is met by a timed wait that cancellation can end, such as a wait on a channel or condition variable. It does not require a clock abstraction or an injected time source.

Owner ruling (2026-11-20), Rand Lee: "No sleep (is async requirement)."

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search of non-test source finds no call to a sleep function.

### Dependencies

**Related:**

- REQ-SAG-0601
- REQ-SAG-0104