# Requirement: TypeSafe API as decidr subcommands

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0501: TypeSafe API as decidr subcommands

**Status:** Approved  

### Requirement Statement

Definitions: `decidr` is the sc-agent decision command-line tool. The TypeSafe API is the Jev decision API (`POST /v1/systemone`, `GET /v1/models`).

**MUST statements:**

- `decidr` MUST expose the TypeSafe API as CLI subcommands.
- `decidr` MUST be able to send any request the TypeSafe API accepts.

### Rationale

Exposing the TypeSafe API as a CLI is requirement #1 for sc-agent: any Jev query can be run from the command line.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each request example in the TypeSafe API documentation (https://docs.typesafe.ai, OpenAPI 0.2.0 at https://api.typesafe.ai/openapi.json) can be sent through a `decidr` subcommand and returns the API's response.
- [ ] The TypeSafe models list is returned by a `decidr` subcommand.

### Dependencies

**Related:**

- REQ-SAG-0201