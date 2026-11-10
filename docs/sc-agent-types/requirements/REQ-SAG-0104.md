# Requirement: Cancel token

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0104: Cancel token

**Status:** Draft  

### Requirement Statement

**MUST statements:**

- `sc-agent-types` MUST provide the cancel token type.
- The cancel token MUST be usable from every sc-agent crate other than `sc-treesitter`, the CLI and the PyO3 bindings.

### Rationale

Streaming and launches in several crates must agree on one cancellation signal.

### Success Criteria

**Acceptance Criteria:**

- [ ] A cancel token created in one crate stops an operation in another crate when cancelled.
- [ ] A cancel token cancelled from Python stops the operation it was passed to.