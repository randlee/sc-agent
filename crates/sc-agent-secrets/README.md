# sc-agent-secrets

API key resolution for the sc-agent workspace.

Keys resolve from the process environment, then the repository-root `.env`, then the platform keyring. Values never appear in output or logs.

Part of the [sc-agent](https://github.com/randlee/sc-agent) workspace. See the
repository `docs/` for architecture and requirements. Licensed under MIT.
