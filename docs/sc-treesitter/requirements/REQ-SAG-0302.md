# Requirement: Any tree-sitter grammar

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0302: Any tree-sitter grammar

**Status:** Approved  

### Requirement Statement

Definitions: A language id is the name a caller uses to select a grammar.

**MUST statements:**

- `sc-treesitter` MUST load tree-sitter grammars and run queries against any loaded grammar.
- The first release MUST support the Python and Rust grammars.
- The second release MUST add the C# grammar, with language id `csharp`, and the Go grammar.

**MUST NOT statements:**

- Adding a grammar MUST NOT require changes to the query API.

### Rationale

The design must support any language; Python and Rust are needed first, C# and Go next.

### Success Criteria

**Acceptance Criteria:**

- [ ] A query runs against Python source and against Rust source through the same API call.
- [ ] After the second release, the same call runs a query against C# source selected as `csharp` and against Go source.
- [ ] A grammar added after release is queried through the existing API with no API change.