# sc-agent-secrets

API key resolution for sc-agent. It sits in the config, secrets and logging layer, above `sc-agent-types`. A key is looked up by its variable name in the process environment, then in the repo-root `.env` file, then in the platform keyring (service = variable name, account = OS user).

The crate logs nothing and never puts key bytes in an error.

## What it provides

- `resolve(provider, &sources)`: the key of a `Provider`, using its variable (`env_var`).
- `resolve_var(&var_name, &sources)`: a key from a variable named by configuration, such as a local runner's `key_env`.
- `KeySources` (`KeySources::system(repo_root)` for the real environment and keyring) with the `EnvSource` and `KeyringSource` traits, so tests pass their own sources. `ProcessEnv` and `PlatformKeyring` are the real ones.
- `ResolvedKey` (the `ApiKey` and its `KeySource`) and `KeySource::as_str`.
- `ApiKey`: a sealed key. `Debug` and `Display` print `[redacted]`. Only `sc-agent-http`'s transport reads it, with `as_str`. `redact_text` removes the key from a string.
- The credential variable names: `TYPESAFE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `TOGETHER_API_KEY`, `NOUS_API_KEY`, `LAYA_API_KEY`, `ALL_VARS` and `env_var`. These are the only credential-name string literals in the workspace.
- `SecretsError`.

## Usage

```rust,ignore
use sc_agent_secrets::{KeySources, resolve};
use sc_agent_types::Provider;

let sources = KeySources::system(None); // None: read no .env file
let resolved = resolve(Provider::Anthropic, &sources)?;
println!("key from {}: {}", resolved.source.as_str(), resolved.key); // prints [redacted]
```

## Depends on

- `sc-agent-types`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). The crate has no runtime. Platform keystores are chosen per target in `Cargo.toml`.

## Requirements and decisions

- REQ-SAG-0401: credential variable names live in the secrets crate.
- REQ-SAG-0002: errors are a discriminated union carried from bottom to top.
- ADR-SAG-0004: crate layering and per-crate error unions.
