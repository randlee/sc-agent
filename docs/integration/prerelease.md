# Owner-run prerelease checks

The prerelease manifest installs and verifies staged wheels through
`scripts/install_wheels.py`. CI uses the generated sc-lint Just integration:
`just setup` installs the released version selected by `sc-lint.toml` (0.8.9),
then `just lint ci all` runs format, native and Windows-target Clippy,
boundary, portability and the full runtime rules. The Clippy toolchain includes
both Clippy and rustfmt. CI uses no sc-lint source checkout or action commit.
Release Preflight keeps the docli freshness check; the live-provider
publication preflight has been removed.


The lead cuts a new template-bearing prerelease after b-12 sanity passes. Use
the **actual tag and source revision** reported by that publish; `prerelease/v0.2.0`
was the input used by plan-anchor and predates the packaged template. Install the
named prerelease with `/prerelease --install` and activate its verified wheel venv.
This keeps the existing prerelease installer and publisher unchanged.

The owner runs the following manually, with credentials available, from the
checkout matching that prerelease. Agents must not run this live command unattended:

```sh
SC_AGENT_LIVE=1 python -m pytest crates/sc-agent-py/tests -m live_outcome -v \
  -o junit_family=legacy --junitxml=/tmp/sc-agent-live.xml
```

The same tests use loopback by default. The live selection covers TypeSafe models
and rendered-consumer noul send/batch outcomes for `jev`, `haiku` and `luna`; the
wheel selects the provider. Only `decidr.no_provider_key` and `secrets.key_missing`
skip an unavailable model. Any other error fails. Nonmarked cases retain the
platform-keyring trap and deterministic loopback data.

`test_invalid_environment_key_auth_cancels_and_redacts` is the single invalid-key
function, parameterized for TypeSafe and Anthropic. In live mode it deliberately
uses an invalid environment key from an isolated non-git directory, checks HTTP
401/auth, cancels the token, verifies a subsequent call returns cancelled, and
checks that results/logs do not disclose the key. It does not skip authentication
failures. Its default mode reuses the existing echo-key loopback fixture. It does
not read a real key or the platform keychain.

Render one report per tested platform from those actual JUnit results:

```sh
python templates/integration-report/render.py /tmp/sc-agent-live.xml \
  --platform macos --mode live --source-revision ACTUAL_SOURCE_SHA \
  --wheel-revision ACTUAL_PRERELEASE_TAG_AND_SHA \
  --output site/reports/integration/macos.xhtml

git add site/reports/integration/macos.xhtml
git commit -m "Record owner-run macOS prerelease results"
git push
```

The renderer converts JUnit into JSON variables, renders the supplied XHTML
section template with sc-compose, and parses the result as XML. It records run
identity, platform, source/wheel revisions, case names, provider/model properties,
PASS/FAIL/SKIP and failure type. Raw traceback/stdout text stays in the local
JUnit file so a failed redaction assertion cannot publish a key. No report is a
publication gate. One passing platform suffices; report skips and failures as
observed. An all-skipped run is not a successful integration run.

For Colima, run the same installed-wheel test command in the local Linux container
with its own Python >=3.11 environment and matching prerelease wheels. Render
with `--platform colima --output site/reports/integration/colima.xhtml`, then
commit and push that report. No SSH or remote Windows setup is required. The
checkout and test dependencies must be available in the container; source and
wheel revisions must describe what that container actually tested.

For deterministic development checks, omit `SC_AGENT_LIVE=1`, use the same
`--junitxml` command, and render with `--mode loopback`. A controlled report is
labelled loopback and is never live evidence.

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
