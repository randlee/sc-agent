# Requirement: PyO3 and CLI query surfaces

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0303: PyO3 and CLI query surfaces

**Status:** Draft  

### Requirement Statement

The `tsq` CLI can expose a subset of the PyO3 query surface.

**MUST statements:**

- The PyO3 bindings MUST accept any query for any loaded grammar.
- The `tsq` CLI MUST provide `languages` and `queries` subcommands.

### Rationale

Python callers need full query access; the CLI runs specific predefined queries that simplify common use.

### Success Criteria

**Acceptance Criteria:**

- [ ] A query written by the caller runs through the PyO3 bindings and returns its matches.
- [ ] `languages` lists the loaded grammars and `queries` lists the predefined queries.