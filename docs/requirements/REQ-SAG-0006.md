# Requirement: No consumer-application types in shared crates

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  

---

## REQ-SAG-0006: No consumer-application types in shared crates

**Status:** Approved  

### Requirement Statement

Definitions: A consumer application is a program built on sc-agent, such as plan-anchor or judge-jev.

**MUST NOT statements:**

- `sc-agent` MUST NOT contain types specific to plan-anchor, judge-jev or any other consumer application.

### Rationale

Many applications will build on sc-agent, some decision-based and some not. Shared crates hold only what every consumer can use.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A review of `sc-agent` finds no type, field or constant named for or used only by a consumer application.

### Product Applicability

**Applies To:**

- sc-agent