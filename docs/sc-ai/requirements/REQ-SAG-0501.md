# Requirement: TypeSafe API as sc-ai subcommands

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0501: TypeSafe API as sc-ai subcommands

**Status:** Draft  

### Requirement Statement

Definitions: sc-ai is the sc-agent command-line tool. The TypeSafe API is the Jev decision API (`POST /v1/systemone`, `GET /v1/models`).

**MUST statements:**

- sc-ai MUST expose the TypeSafe API as CLI subcommands.
- sc-ai MUST be able to send any request the TypeSafe API accepts.

### Rationale

Exposing the TypeSafe API as a CLI is requirement #1 for sc-agent: any Jev query can be run from the command line.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each request example in the TypeSafe API documentation can be sent through an sc-ai subcommand and returns the API's response.
- [ ] The TypeSafe models list is returned by an sc-ai subcommand.

### Dependencies

**Related:**

- REQ-SAG-0201