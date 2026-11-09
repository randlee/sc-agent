# Requirement: Any tree-sitter grammar

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0302: Any tree-sitter grammar

**Status:** Draft  

### Requirement Statement

**MUST statements:**

- `sc-treesitter` MUST load tree-sitter grammars and run queries against any loaded grammar.
- The C# (.NET) and Go grammars MUST be supported early.

**MUST NOT statements:**

- Adding a grammar MUST NOT require changes to the query API.

### Rationale

The design must support any language; .NET and Go are the first needed.

### Success Criteria

**Acceptance Criteria:**

- [ ] A query runs against C# source and against Go source through the same API call.
- [ ] A grammar added after release is queried through the existing API with no API change.