# Requirement: Credential resolution order and keyring defaults

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0401: Credential resolution order and keyring defaults

**Status:** Approved  

### Requirement Statement

Definitions: A *provider* is a language-model service whose API key `sc-agent-secrets` resolves: TypeSafe, Anthropic or OpenAI. A *credential* is the API key for one provider. A *credential-variable name* is the name of the environment variable that holds a provider's credential. The *keyring service* and the *keyring account* are the two components that identify a provider's entry in the platform keyring; callers override them with the `keyring_service` and `keyring_account` options. The *resolution order* is the process environment, then the repository-root `.env` file, then the platform keyring (the OS credential store: Keychain on macOS, Credential Manager on Windows, Secret Service on Linux). A credential is *present* in a source when the source holds a value under the provider's credential-variable name; a blank platform-keyring value counts as absent. A *keyring override* is a `keyring_service` or `keyring_account` option passed to a call; its value is a *keyring component*. To *refuse* a call is to return an error from it before it performs any network I/O. `sc_agent_py.invalid_option` is the error identifier the Python bindings (`sc-agent-py`) return for an invalid option. *Automatic provider selection* is the behaviour of a call with `provider=None`: it ranks the providers whose keys are detected. An *invalid-option refusal* is a refusal whose error identifier is `sc_agent_py.invalid_option`. The *current OS account* is the user name of the operating-system user running the process.

**MUST statements:**

- `sc-agent-secrets` MUST resolve a key from the sources in the resolution order.
- The first source that has the variable MUST win.

- A caller may pass a keyring service and account; they replace the defaults for that call.
- When a call omits the keyring service, the keyring service MUST default to that provider's credential-variable name.
- When a call omits the keyring account, the keyring account MUST default to the current OS account.

### Rationale

Credential handling stays in one crate, so redaction and key lookup cannot disagree about which variables hold secrets.

Owner ruling (2026-11-20), Rand Lee: "Keyring is super basic." The keyring account defaults to the current user account unless something else is passed in.

Owner ruling (2026-11-20), Rand Lee: "Isn't ENV > .env > keyring (or win/linux equivalent) standard order?" A variable already set in the environment wins over `.env`, which is also dotenvy's default.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "Keyring is super basic."

### Success Criteria

**Acceptance Criteria:**

- [ ] With a valid key in the environment, a different valid key in the repository-root `.env` and another in the keyring, the environment key is used; with none in the environment, the `.env` key is used; with neither, the keyring key is used.