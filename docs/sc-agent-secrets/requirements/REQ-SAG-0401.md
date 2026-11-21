# Requirement: Credential variable names live in the secrets crate

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0401: Credential variable names live in the secrets crate

**Status:** Approved  

### Requirement Statement

Definitions: A *provider* is any provider in the providers table, the `Provider` enum of `sc-agent-types` (REQ-SAG-0103); `sc-agent-secrets` resolves each one's API key. A *credential* is the API key for one provider. A *credential-variable name* is the name of the environment variable that holds a provider's credential. The *keyring service* and the *keyring account* are the two components that identify a provider's entry in the platform keyring; callers override them with the `keyring_service` and `keyring_account` options. The *resolution order* is the process environment, then the repository-root `.env` file, then the platform keyring (the OS credential store: Keychain on macOS, Credential Manager on Windows, Secret Service on Linux). A credential is *present* in a source when the source holds a value under the provider's credential-variable name; a blank platform-keyring value counts as absent. An *invalid credential* is a present value that the key rules of `sc-agent-secrets` (`ApiKey::try_new`) refuse. The *current OS account* is the user name of the operating-system user running the process.

**MUST statements:**

- Credential environment-variable names such as `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` MUST be defined in `sc-agent-secrets`.
- When a crate other than `sc-agent-secrets` uses a credential variable name, that crate MUST obtain the name from `sc-agent-secrets`.
- `sc-agent-secrets` MUST resolve a key from the sources in the resolution order.

- When a source earlier in the resolution order holds a present but invalid credential, `sc-agent-secrets` MUST treat that credential as authoritative.

- A caller may pass a keyring service and account; they replace the defaults for that call.
- When a call omits the keyring service, the keyring service MUST default to that provider's credential-variable name.
- When a call omits the keyring account, the keyring account MUST default to the current OS account.

**MUST NOT statements:**

- When a source earlier in the resolution order holds a present but invalid credential, `sc-agent-secrets` MUST NOT fall back to a later source in the resolution order.

### Rationale

Credential handling stays in one crate, so redaction and key lookup cannot disagree about which variables hold secrets.

Owner ruling (2026-11-20), Rand Lee: "Keyring is super basic." The keyring account defaults to the current user account unless something else is passed in.

Owner ruling (2026-11-20), Rand Lee: "Isn't ENV > .env > keyring (or win/linux equivalent) standard order?" A variable already set in the environment wins over `.env`, which is also dotenvy's default.

### Success Criteria

**Acceptance Criteria:**

- [ ] A workspace search finds credential variable names as string literals only in `sc-agent-secrets`.
- [ ] With a valid key in the environment, a different valid key in the repository-root `.env` and another in the keyring, the environment key is used; with none in the environment, the `.env` key is used; with neither, the keyring key is used.