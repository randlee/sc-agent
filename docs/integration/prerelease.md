# Owner-run prerelease checks

The prerelease manifest installs and verifies staged wheels through
`scripts/install_wheels.py`. CI installs sc-lint with
`randlee/sc-lint@v1` at the version in `sc-lint.toml` and runs
`sc-lint lint ci --consumer`, including the production panic rule.
Release Preflight keeps the docli freshness check; the live-provider
publication preflight has been removed.


Install the intended prerelease with `/prerelease --install` and activate its
`venv`. Run the existing wheel tests from the checkout against that installed
wheel, with the owner present:

```sh
SC_AGENT_LIVE=1 python -m pytest crates/sc-agent-py/tests/test_api.py -m live_outcome -v
```

This explicitly uses the owner's real configuration and credentials. It checks
one default TypeSafe models list and noul `send`/`send_batch` outcomes for `jev`,
`haiku` and `luna`. The wheel selects each model's actual provider; no provider is
forced. A native missing-key error skips that model. Any other error fails.

Without `SC_AGENT_LIVE=1`, the same outcomes use the loopback runner. All
nonmarked tests remain loopback-only, including request inspection, fixed-answer
and local-log assertions. The platform-keyring trap remains armed for them.

These are hand-run product checks. No integration evidence, report or publication
gate is produced. Actual credentialed runs require lead coordination and owner
attendance; agents must not run this live command unattended.

The pinned sc-publish kit still carries its legacy sc-lint source installer;
[sc-publish#187](https://github.com/randlee/sc-publish/issues/187) tracks its
removal. Retained kit files include `.github/scripts/setup_sc_lint_source.py`,
`.github/scripts/tests/test_setup_sc_lint_source.py`,
`.github/scripts/tests/test_release_artifacts.py` and
`.github/scripts/tests/test_npm_release.py`. Some still reference the removed
`.github/actions/setup-sc-lint/action.yml`; the source-setup tests therefore
currently fail on that missing file. These files await a pinned kit fix and
installer regeneration; do not hand-edit them. CI uses the official sc-lint
action above, independently of that retained release-kit path.
