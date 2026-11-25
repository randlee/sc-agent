# Requirement: PyO3 and CLI query surfaces

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0303: PyO3 and CLI query surfaces

**Status:** Approved  

### Requirement Statement

Definitions: CLI means command-line interface. A loaded grammar is a tree-sitter grammar loaded by `sc-treesitter`. The PyO3 bindings expose the Python query interface to `sc-treesitter`.

The `sitr` CLI can expose a subset of the PyO3 query surface.

**MUST statements:**

- The `sc-treesitter` Python bindings MUST accept any tree-sitter query for any loaded tree-sitter grammar.
- The `sitr` CLI MUST provide the `languages` subcommand.
- The `sitr` CLI MUST provide the `queries` subcommand.
- The `sitr` CLI MUST provide a `--version` flag.

### Rationale

Python callers need full query access; the CLI runs specific predefined queries that simplify common use.

Owner ruling (2026-11-21), Rand Lee: every CLI has `--version`.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A query written by the caller runs through the PyO3 bindings and returns its matches.
- [ ] `languages` lists the loaded grammars and `queries` lists the predefined queries.
- [ ] `sitr --version` prints the version.