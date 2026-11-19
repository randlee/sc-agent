# Requirement: Credential variable names live in the secrets crate

**Status:** Approved  
**Version:** 0.1.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0401: Credential variable names live in the secrets crate

**Status:** Approved  

### Requirement Statement

Definitions: A *provider* is a language-model service whose API key `sc-agent-secrets` resolves: TypeSafe, Anthropic or OpenAI. A *credential* is the API key for one provider. A *credential-variable name* is the name of the environment variable that holds a provider's credential. The *keyring service* and the *keyring account* are the two components that identify a provider's entry in the platform keyring; callers override them with the `keyring_service` and `keyring_account` options. The *resolution order* is the repository-root `.env` file, then the process environment, then the platform keyring. A credential is *present* in a source when the source holds a value under the provider's credential-variable name; a blank platform-keyring value counts as absent. An *invalid credential* is a present value that the key rules of `sc-agent-secrets` (`ApiKey::try_new`) refuse. A *keyring override* is a `keyring_service` or `keyring_account` option passed to a call; its value is a *keyring component*. To *refuse* a call is to return an error from it before it performs any network I/O. `sc_agent_py.invalid_option` is the error identifier the Python bindings (`sc-agent-py`) return for an invalid option. *Automatic provider selection* is the behaviour of a call with `provider=None`: it ranks the providers whose keys are detected. An *invalid-option refusal* is a refusal whose error identifier is `sc_agent_py.invalid_option`. The *current OS account* is the user name of the operating-system user running the process.

**MUST statements:**

- Credential environment-variable names such as `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` MUST be defined in `sc-agent-secrets`.
- When a crate other than `sc-agent-secrets` uses a credential variable name, that crate MUST obtain the name from `sc-agent-secrets`.
- `sc-agent-secrets` MUST resolve a key from the sources in the resolution order.

- When a source earlier in the resolution order holds a present but invalid credential, `sc-agent-secrets` MUST treat that credential as authoritative.

- Callers MUST be able to override the keyring service independently per provider.
- Callers MUST be able to override the keyring account independently per provider.
- When a call omits the keyring service, the keyring service MUST default to that provider's credential-variable name.
- When a call omits the keyring account, the keyring account MUST default to the current OS account.
- The length of each provided keyring component MUST lie in the range 1–255 UTF-8 bytes.
- When a provided keyring component is outside 1–255 UTF-8 bytes, the call MUST be refused.
- When a call is refused for a keyring component outside 1–255 UTF-8 bytes, the error MUST name the option that carried the component.
- When a call is refused for a keyring component outside 1–255 UTF-8 bytes, the error MUST name the bound of 1–255 UTF-8 bytes.
- When a Python call supplies a keyring override, the call MUST name `provider`.
- When a Python call supplies a keyring override without naming `provider`, the call MUST end in an invalid-option refusal.
- When a Python call supplies no keyring override, the call MUST retain automatic provider selection.

**MUST NOT statements:**

- When a source earlier in the resolution order holds a present but invalid credential, `sc-agent-secrets` MUST NOT fall back to a later source in the resolution order.
- When a call is refused for a keyring component outside 1–255 UTF-8 bytes, the error MUST NOT retain the supplied value.
- Overrides for one provider MUST NOT affect another provider.

### Rationale

Credential handling stays in one crate, so redaction and key lookup cannot disagree about which variables hold secrets.

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search finds credential variable names as string literals only in `sc-agent-secrets`.