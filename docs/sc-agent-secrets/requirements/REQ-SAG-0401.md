# Requirement: Credential variable names live in the secrets crate

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0401: Credential variable names live in the secrets crate

**Status:** Draft  

### Requirement Statement

**MUST statements:**

- Credential environment-variable names such as `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` MUST be defined in `sc-agent-secrets`.
- Any other crate that needs a credential variable name MUST obtain it from `sc-agent-secrets`.
- `sc-agent-secrets` MUST resolve a key from the process environment first, then the repository-root `.env` file, then the platform keyring.

### Rationale

Credential handling stays in one crate, so redaction and key lookup cannot disagree about which variables hold secrets.

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search finds credential variable names as string literals only in `sc-agent-secrets`.