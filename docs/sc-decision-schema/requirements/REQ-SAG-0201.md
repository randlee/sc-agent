# Requirement: Jev API types live in sc-decision-schema

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0201: Jev API types live in sc-decision-schema

**Status:** Draft  

### Requirement Statement

Definitions: The Jev API is the TypeSafe decision API (`POST /v1/systemone`, `GET /v1/models`) with the `choice`, `score`, `noul` and `box` question and answer types.

**MUST statements:**

- Every type that maps to the Jev API MUST be defined in `sc-decision-schema`.
- `sc-decision-schema` MUST cover 100% of the Jev API request and response.

### Rationale

The first requirement for sc-agent is to expose the TypeSafe API, so any Jev query can be run through it.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each request and response example in the TypeSafe API documentation (https://docs.typesafe.ai, OpenAPI 0.2.0 at https://api.typesafe.ai/openapi.json), for all four question types, round-trips through `sc-decision-schema` types without loss.
- [ ] No crate other than `sc-decision-schema` defines a type that maps to a Jev API field.

### Dependencies

**Related:**

- ADR-SAG-0001