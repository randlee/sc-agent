# Owner-run prerelease checks

The selected candidate's version comes from `[workspace.package].version` in
[Cargo.toml](../../Cargo.toml), read at that candidate revision. Use the actual
published tag and commit when selecting a wheel; do not copy a release number
into these instructions. Consumers keep their wheel pins in their own release
configuration and render the client using the installed wheel's metadata.

The release kit revision is pinned separately in
[release/sc-publish-pin.toml](../../release/sc-publish-pin.toml).
[The prerelease skill](../../.codex/skills/prerelease/SKILL.md) describes asset
selection, installation and checksum verification. Staged-wheel verification
uses [scripts/install_wheels.py](../../scripts/install_wheels.py).

## Local verification

Install the build tools and versions declared by
[CI](../../.github/workflows/ci.yml), then run:

```sh
python scripts/gate/rewrite.py
python scripts/check_consumer_template.py
bash scripts/generate-cli-docs.sh
git diff --exit-code -- fixtures/cli/ site/cli/
```

The gate builds and installs the current wheel, runs Rust and wheel/CLI
contracts, and exercises both rendered consumers against loopback runners.
The type check covers both generated clients. CLI reference freshness uses the
pin in [release/docli-pin.toml](../../release/docli-pin.toml) and remains part of
release preflight. CI runs Cargo formatting and Clippy directly.

## Consumer integration and credential export

The plan-anchor and judge-jev suites verify their installed wheel. Their own
owner-attended live runs cover real providers:
[plan-anchor](https://github.com/randlee/plan-anchor/blob/develop/docs/integration.md)
and [judge-jev](https://github.com/randlee/judge-jev/blob/develop/docs/integration/owner-attended-live-tests.md).
Those consumers own wheel selection, execution and reports.

[scripts/integration.py](../../scripts/integration.py) only exports credentials:

```sh
KEY_DIR=$(mktemp -d /tmp/sc-agent-keys.XXXXXX)
trap 'rm -rf "$KEY_DIR"' EXIT
python3 scripts/integration.py --export-env "$KEY_DIR/keys.env"
# Pass this file to the consumer's documented --env-file invocation.
```

Run the export with the owner present on macOS. It reads the normal keychain
entry using the provider's credential-variable name as service and the current
OS user as account, falling back to the environment when the entry is absent.
Names come from the Rust provider table. At least one of TYPESAFE_API_KEY, ANTHROPIC_API_KEY or NOUS_API_KEY is required
(haiku and luna need no TypeSafe key); every available key is written to a new mode-0600 file outside the checkout and never printed.
Existing files are not overwritten. Delete the temporary file after the run;
never commit it. The exporter does not build wheels, run suites or render reports.

For a candidate exposing the Run API, consumer checks select `haiku` or `luna`
without `TYPESAFE_API_KEY`. Use `with sc_agent.Run(tool=..., model=..., project=...,
agent_role=...) as run` and inspect `run.sends_to(sc_agent.TYPESAFE)` for TypeSafe
consent. Timeout and workers belong to that Run. Credential export accepts any
keyed provider returned by `decidr models` and exits 2 before writing when none
are available. Live calls remain owner-attended through the consumer suites above.
