# Requirement: Any tree-sitter grammar

**Status:** Approved  
**Version:** 0.1.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0302: Any tree-sitter grammar

**Status:** Approved  

### Requirement Statement

Definitions: A language id is the name a caller uses to select a grammar. A tree-sitter grammar describes a language for parsing. The query API is the `sc-treesitter` interface for running tree-sitter queries against a loaded grammar. API means application programming interface. To load a grammar is to obtain the tree-sitter grammar selected by its language id so that the query API can run queries against it. A release supports a grammar when `sc-treesitter` in that release loads the grammar and runs tree-sitter queries against source in that grammar's language through the query API.

**MUST statements:**

- `sc-treesitter` MUST load tree-sitter grammars.
- `sc-treesitter` MUST run queries against any loaded grammar.
- The first release MUST support the Python grammar.
- The first release MUST support the Rust grammar.

**MUST NOT statements:**


### Rationale

The design must support any language; Python and Rust are needed first, C# and Go next.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that a query runs against Python source and against Rust source through the same API call.
- [ ] A unit test verifies that after the second release the same call runs a query against C# source selected as `csharp` and against Go source.
- [ ] A unit test verifies that a grammar added after release is queried through the existing API with no API change.