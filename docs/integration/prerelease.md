# Owner-run prerelease checks

The prerelease manifest installs and verifies staged wheels through
`scripts/install_wheels.py`. CI uses the generated sc-lint Just integration:
`just setup` installs the released version selected by `sc-lint.toml`,
then `just lint ci all` runs format, native and Windows-target Clippy,
boundary, portability and the full runtime rules. The Clippy toolchain includes
both Clippy and rustfmt. CI uses no sc-lint source checkout or action commit.
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
