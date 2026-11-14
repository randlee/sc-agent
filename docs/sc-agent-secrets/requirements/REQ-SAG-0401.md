# Requirement: Credential variable names live in the secrets crate

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0401: Credential variable names live in the secrets crate

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- Credential environment-variable names such as `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` MUST be defined in `sc-agent-secrets`.
- Any other crate that needs a credential variable name MUST obtain it from `sc-agent-secrets`.
- `sc-agent-secrets` MUST resolve a key from the repository-root `.env` file first, then the process environment, then the platform keyring.

- A present but invalid credential in a higher-priority source MUST be authoritative and MUST NOT fall back to a lower-priority source.

- Callers MUST be able to override keyring service and account independently per provider. An omitted service MUST default to that provider's credential-variable name; an omitted account MUST default to the current OS account.
- Each provided keyring component MUST contain 1–255 UTF-8 bytes, validated before network I/O. Errors MUST name the option and bound without retaining its supplied value.
- Overrides for one provider MUST NOT affect another provider.

- Python calls supplying either keyring override MUST name `provider`; otherwise they MUST return `sc_agent_py.invalid_option` before network I/O. Calls without overrides MUST retain automatic provider selection.

### Rationale

Credential handling stays in one crate, so redaction and key lookup cannot disagree about which variables hold secrets.

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search finds credential variable names as string literals only in `sc-agent-secrets`.