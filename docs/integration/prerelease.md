# Owner-run prerelease checks

The prerelease manifest installs and verifies staged wheels through
`scripts/install_wheels.py`. CI uses the generated sc-lint Just integration:
`just setup` installs the released version selected by `sc-lint.toml` (0.8.9),
then `just lint ci all` runs format, native and Windows-target Clippy,
boundary, portability and the full runtime rules. The Clippy toolchain includes
both Clippy and rustfmt. CI uses no sc-lint source checkout or action commit.
Release Preflight keeps the docli freshness check; the live-provider
publication preflight has been removed.


## Repeatable integration sequence

After the usual wheel dev install and `cargo build -p decidr`, unit tests are
always loopback: `python -m pytest crates/sc-agent-py/tests`.
Real integration is a separate suite with no loopback/live switch. First the dev
runs the local build and fixes every failure:

```sh
just integration --results /tmp/sc-agent-local-results
```

The command creates a fresh Python 3.11 venv, installs maturin/pytest, builds and
installs this checkout's wheel, renders its shipped consumer, and runs the same
suite once per provider. TypeSafe uses `jev`, Anthropic uses `haiku`, OpenAI uses
`luna`. Each run sends a meaningful noul question singly and in a batch, then
checks invalid-key HTTP401/auth, cancellation, a cancelled follow-up and redaction.
Provider selection stays automatic; the consumer never forces a provider.

On macOS the small runner reads each provider's normal keychain entry (service
from sc-agent-secrets, account the OS user), falling back to its environment key
when the entry is absent. It never prints a key or asks for one. Each child runs
outside a git repository in a throwaway environment containing only its provider's
credential; no repository `.env` can override it. A temporary `.env` can instead
be supplied with `--env-file PATH`: it is consumed and deleted in `finally`, even
when build/tests fail. No credentials are committed. TypeSafe absence fails the
command; another provider with no key is explicitly NOT RUN in its named JUnit.
Any service/test error fails, and an available provider whose suite is entirely
skipped fails. Keychain errors other than a missing item fail.

To test selected binaries without building, use the same command:

```sh
just integration --wheel /absolute/path/to/sc_decidr-WHEEL.whl --results /tmp/wheel-results
just integration --venv /absolute/path/to/prerelease-venv --results /tmp/venv-results
```

The local-build run produces per-case JUnit results, **no XHTML**. Once it passes,
the lead cuts `/prerelease` from the fixed head and records the actual tag and SHA.
The version selected by the publisher may differ from the used input tag
`prerelease/v0.2.0`. Existing prerelease install/publish and checksum verification
remain unchanged. Install/verify the selected published wheel with
`/prerelease --install X.Y.Z` before the clean-account run below.

## Clean prerelease run on atmbench

Use `ssh atmbench@rand-m5` (not a machine-local alias). The account's system Python
is 3.9: stage the existing standalone uv and sc-compose binaries, then install a
managed Python 3.11 and all packages **inside a disposable directory**. Supply
keys only in a mode-0600 temporary `.env`; the runner never reads the keychain in
`--env-file` mode. The same provider suite generates one XHTML panel per provider
and platform from that run's JUnit, with the prerelease tag/SHA recorded as its pin.

Set `PIN_TAG`, `PIN_SHA`, and `WHEEL` to the actual published/checksum-verified
candidate. Run this from the checkout containing that source revision. The archive
uses that exact pin, not a dirty working tree. All remote installs, caches, sources,
credentials and test temporaries are removed on exit, including failure; only the
rendered report panels return to this checkout.

```bash
set -euo pipefail
KEY_DIR=$(mktemp -d /tmp/sc-agent-keys.XXXXXX)
CLEAN_ROOT=""
cleanup() {
  rm -rf "$KEY_DIR"
  if [ -n "$CLEAN_ROOT" ]; then
    ssh -o BatchMode=yes atmbench@rand-m5 "rm -rf '$CLEAN_ROOT'"
  fi
}
trap cleanup EXIT
python3 scripts/integration.py --export-env "$KEY_DIR/keys.env"
CLEAN_ROOT=$(ssh -o BatchMode=yes atmbench@rand-m5 'mktemp -d /tmp/sc-agent-clean.XXXXXX')
ssh -o BatchMode=yes atmbench@rand-m5 "mkdir -p '$CLEAN_ROOT/bin' '$CLEAN_ROOT/source'"
git archive "$PIN_SHA" | ssh -o BatchMode=yes atmbench@rand-m5 "tar -xf - -C '$CLEAN_ROOT/source'"
scp -o BatchMode=yes "$(command -v uv)" "$(command -v sc-compose)" "atmbench@rand-m5:$CLEAN_ROOT/bin/"
scp -o BatchMode=yes "$WHEEL" "atmbench@rand-m5:$CLEAN_ROOT/"
scp -o BatchMode=yes "$KEY_DIR/keys.env" "atmbench@rand-m5:$CLEAN_ROOT/keys.env"
rm -f "$KEY_DIR/keys.env"
ssh -o BatchMode=yes atmbench@rand-m5 sh -s -- "$CLEAN_ROOT" "$PIN_TAG" "$PIN_SHA" <<'REMOTE' | tar -xf -
set -eu
scratch=$1
pin_tag=$2
pin_sha=$3
trap 'rm -rf "$scratch"' EXIT
export UV_PYTHON_INSTALL_DIR="$scratch/python"
export UV_CACHE_DIR="$scratch/cache"
export TMPDIR="$scratch/tmp"
export PATH="$scratch/bin:$PATH"
mkdir -p "$TMPDIR"
uv python install 3.11 --no-bin >&2
uv venv --managed-python --python 3.11 --seed "$scratch/venv" >&2
wheel=$(find "$scratch" -maxdepth 1 -name '*.whl')
uv pip install --python "$scratch/venv/bin/python" "$wheel" pytest >&2
cd "$scratch/source"
status=0
"$scratch/venv/bin/python" scripts/integration.py --venv "$scratch/venv" \
  --env-file "$scratch/keys.env" --results "$scratch/results" --platform macos \
  --pin-tag "$pin_tag" --pin-sha "$pin_sha" >&2 || status=$?
tar -cf - site/reports/integration
exit "$status"
REMOTE

git add site/reports/integration/macos-*.xhtml
git commit -m "Record clean-account prerelease integration results"
git push
```

The local `.env` export and remote copy are deleted on success and failure; the
remote disposable tree also removes Python/uv/sc-compose/venv installations and
caches. No system Python, shell profile or pre-existing account files are changed.
The report renderer excludes raw traceback/stdout text so a failed redaction
assertion cannot publish a key. FAIL and NOT RUN are retained as observed; neither
is presented as a passing case.

For Colima, repeat the **same suite command** with the published Linux wheel or
installed venv and a temporary `.env`, add `--platform colima --pin-tag TAG
--pin-sha SHA`, and commit/push `colima-*.xhtml`. Use a disposable container/volume
and remove it and its temporary credentials after collecting the panels. One
passing platform suffices; there is no automatic publication gate or CI live run.

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
