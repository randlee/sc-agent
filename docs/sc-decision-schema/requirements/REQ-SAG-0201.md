# Requirement: Jev API types live in sc-agent

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0201: Jev API types live in sc-agent

**Status:** Approved  

### Requirement Statement

Definitions: The Jev API is the TypeSafe decision API (`POST /v1/systemone`, `GET /v1/models`) with the `choice`, `score`, `noul` and `box` question and answer types.

**MUST statements:**

- Every type that maps to the Jev API MUST be defined in `sc-agent`.
- `sc-agent` MUST cover 100% of the Jev API request and response.

### Rationale

The first requirement for sc-agent is to expose the TypeSafe API, so any Jev query can be run through it.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each request and response example in the TypeSafe API documentation (https://docs.typesafe.ai, OpenAPI 0.2.0 at https://api.typesafe.ai/openapi.json), for all four question types, round-trips through `sc-agent` without loss.
- [ ] No crate other than `sc-agent` defines a type that maps to a Jev API field.

### Dependencies

**Related:**

- ADR-SAG-0001