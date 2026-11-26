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

Definitions: A consumer application is any program, other than sc-agent itself, that depends on sc-agent; plan-anchor and judge-jev are two. A consumer-specific item is a Rust type, field or constant whose name includes the name of a consumer application, or that exactly one consumer application uses.

**MUST NOT statements:**

- A crate in the `crates/` folder MUST NOT contain a consumer-specific item.

### Rationale

Many applications will build on sc-agent, some decision-based and some not. Shared crates hold only what every consumer can use.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A review of `sc-agent` finds no type, field or constant named for or used only by a consumer application.

### Product Applicability

**Applies To:**

- sc-agent