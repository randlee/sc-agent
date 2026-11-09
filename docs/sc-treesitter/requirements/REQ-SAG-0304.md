# Requirement: Git use is limited to the repository name

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0304: Git use is limited to the repository name

**Status:** Draft  

### Requirement Statement

**MUST statements:**

- `sc-treesitter` MUST read the source it parses from the file path it is given.

**MUST NOT statements:**

- `sc-treesitter` MUST NOT use git for anything other than determining the repository name.

### Rationale

The owner's only use for git here is deriving the project name from the repository.

### Success Criteria

**Acceptance Criteria:**

- [ ] Parsing a file outside any git repository succeeds.
- [ ] `sc-treesitter` runs no git command other than one that reads the repository name.