# Requirement: No consumer-application types in shared crates

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0006: No consumer-application types in shared crates

**Status:** Approved  

### Requirement Statement

Definitions: A consumer application is a program built on sc-agent, such as plan-anchor or judge-jev. A mock backend is a model, such as Haiku or Luna, answering in place of Jev.

**MUST NOT statements:**

- `sc-agent` MUST NOT contain types specific to plan-anchor, judge-jev or any other consumer application.
- `sc-agent` MUST NOT contain types that describe Haiku, Luna or any other model acting as a Jev mock.

### Rationale

Many applications will build on sc-agent, some decision-based and some not. Shared crates hold only what every consumer can use; a backend's knowledge of how it adapts a request belongs to that backend.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A review of `sc-agent` finds no type, field or constant named for or used only by a consumer application.
- [ ] `sc-agent` has no item that refers to one model standing in for another.

### Product Applicability

**Applies To:**

- sc-agent