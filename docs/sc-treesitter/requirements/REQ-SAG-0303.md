# Requirement: PyO3 and CLI query surfaces

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0303: PyO3 and CLI query surfaces

**Status:** Approved  

### Requirement Statement

The `sitr` CLI can expose a subset of the PyO3 query surface.

**MUST statements:**

- The PyO3 bindings MUST accept any query for any loaded grammar.
- The `sitr` CLI MUST provide `languages` and `queries` subcommands and a `--version` flag.

### Rationale

Python callers need full query access; the CLI runs specific predefined queries that simplify common use.

Owner ruling (2026-11-21), Rand Lee: every CLI has `--version`.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A query written by the caller runs through the PyO3 bindings and returns its matches.
- [ ] `languages` lists the loaded grammars and `queries` lists the predefined queries.
- [ ] `sitr --version` prints the version.