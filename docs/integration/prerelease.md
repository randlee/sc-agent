# Owner-run prerelease checks

The prerelease manifest installs and verifies staged wheels through
`scripts/install_wheels.py`. CI uses the generated sc-lint Just integration:
`just setup` installs the released version selected by `sc-lint.toml`,
then `just lint ci all` runs format, native and Windows-target Clippy,
boundary, portability and the full runtime rules. The Clippy toolchain includes
both Clippy and rustfmt. CI uses no sc-lint source checkout or action commit.
Release Preflight keeps the docli freshness check; the live-provider
publication preflight has been removed.


## Consumer integration and credential export

The rewrite is verified by the plan-anchor and judge-jev suites against the
installed wheel. Their own owner-attended live runs cover real providers; the
old sc-agent wheel-building and provider-suite runner has been removed.
See [plan-anchor's integration instructions](https://github.com/randlee/plan-anchor/blob/develop/docs/integration.md)
and [judge-jev's owner-attended live tests](https://github.com/randlee/judge-jev/blob/develop/docs/integration/owner-attended-live-tests.md).

`scripts/integration.py` now only exports credentials for those consumer runs:

```sh
KEY_DIR=$(mktemp -d /tmp/sc-agent-keys.XXXXXX)
trap 'rm -rf "$KEY_DIR"' EXIT
python3 scripts/integration.py --export-env "$KEY_DIR/keys.env"
# Pass this temporary file to the consumer's documented --env-file invocation.
```

Run the export with the owner present on macOS: it reads the normal keychain
entry using the provider's credential-variable name as service and the current
OS user as account, falling back to the environment when the entry is absent.
Provider credential names come from the current Rust provider table. The command
requires a TypeSafe key, writes only available keys, never prints them, and
creates a new file outside the checkout with mode 0600. Existing files are not
overwritten. Delete the temporary file after the consumer run; never commit it.

The command does not build or select a wheel, run a provider suite, or render a
report. Its former `--wheel`, `--venv`, `--python`, `--env-file`, `--platform`,
`--results`, `--pin-tag` and `--pin-sha` options are removed. Consumers own the
wheel selection, test execution and reports described in their integration docs.

The pinned sc-publish kit still carries its legacy sc-lint source installer;
[sc-publish#187](https://github.com/randlee/sc-publish/issues/187) tracks its
removal. Retained kit files include `.github/scripts/setup_sc_lint_source.py`,
`.github/scripts/tests/test_setup_sc_lint_source.py`,
`.github/scripts/tests/test_release_artifacts.py` and
`.github/scripts/tests/test_npm_release.py`. Some still reference the removed
`.github/actions/setup-sc-lint/action.yml`; the source-setup tests therefore
currently fail on that missing file. These files await a pinned kit fix and
installer regeneration; do not hand-edit them. CI uses the generated sc-lint Just integration above, independently of that retained release-kit path.

Boundary records intentionally list allowed internal dependents by crate name.
The developer adding or changing a Cargo dependency updates both affected
boundary records in the same change: the consumer's allowed dependencies and
the dependency owner's allowed dependents (using dev entries for dev-only
edges). Derive these entries from ADR-SAG-0004 and the actual Cargo graph.

The shared renderer writes each run's XHTML panels and JSON under
`site/reports/<run-id>/`, embeds those panels in `site/reports/<run-id>.html`, and
updates `site/reports/index.html` newest first. Each provider in one invocation
shares a run id. Existing XHTML can use the same publisher without rerendering:

```bash
python templates/integration-report/render.py --panel path/to/panel.xhtml \
  --reports-dir site/reports --run-id YYYYMMDD-1-integration --title "Integration report"
```

Repeat `--panel` for multiple panels. Reusing the run id updates its page and
retains one index entry. The site home page's Reports link is installed once.
