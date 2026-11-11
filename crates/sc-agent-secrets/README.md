# sc-agent-secrets

API key resolution for sc-agent. A key is looked up by its variable name in the
process environment, then the repo-root `.env` file, then the platform keyring
(service = variable name, account = OS user).

- `resolve(provider, &sources)` uses the provider's variable (`env_var`).
- `resolve_var(&var_name, &sources)` uses a variable named by configuration,
  such as a local runner's `key_env`.
- `KeySources::system(repo_root)` reads the real environment and keyring;
  tests pass their own `EnvSource` and `KeyringSource`.

`ApiKey` prints `[redacted]`; only sc-agent-http's transport reads it. The
crate logs nothing and depends only on sc-agent-types, keyring and whoami
(no tokio).
