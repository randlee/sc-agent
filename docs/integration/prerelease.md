# Prerelease provider integration

The run uses the installed prerelease's `decidr` binary and `sc-decidr` wheel.
It verifies the target binary archive and both host wheels against the release's
`checksums.txt` before installing them into an isolated environment. Python 3.11+
is required. The integration formula contains ten live cases: TypeSafe model
discovery, TypeSafe noul decisions, Anthropic structured-output decisions, and
invalid-credential/redaction checks for each provider, through both interfaces.

Provider, model, interface, operation and result expectation are configured per case
in the formula’s `steps.metadata.integration` tables. Platform identifiers come
from declared wheel IDs or target triples in `release/publish-artifacts.toml`;
architecture-spelled aliases are derived from those declarations. Wheel package
names and import modules come from the same manifest.

The case-result boundary is implemented in
`crates/decidr/src/integration_gate.rs`. The installed wheel exposes the same
validator through `validate_integration_results(results_json, source_commit)`;
`integration_cases()` supplies the required case names. These APIs are additive.
The producer checks the installed case list against the formula before execution.

## Prepare a candidate

Change `release/sc-publish-consumer-input.json`, then run the installer from the
revision in `release/sc-publish-pin.toml`. Do not edit the generated release
manifests or workflows. The pinned installer must report no drift on a repeat
`--dry-run`.

Both Python projects declare `requires-python = ">=3.11"`; their CI jobs build,
install and test the wheels on Python 3.11. The Rust ABI may support an earlier
Python version, but package metadata rejects installation below the supported
floor.

The manifest's `scripts/prerelease_tag.py` owns version selection. It refuses a
dirty, protected or detached branch and accepts no explicit version. It uses an
unused workspace version or patch-bumps past existing stable/prerelease versions,
updates workspace package/path-dependency and Python package versions, and pushes
a new annotated tag without force. The manifest-declared prerelease helper waits
for all release targets, wheels, sdists and checksums. Publication requires the
operator's explicit `--publish` authorization; a dry run changes nothing:

```sh
python3 .claude/skills/prerelease/scripts/prerelease.py --publish --dry-run
```

## Execute a platform

Use an attached branch checked out at the exact prerelease source. It must have
access to `git`, authenticated `gh`, `bd`, sc-compose 1.8+, Python 3.11+ and the
provider credentials. Configure TypeSafe and Anthropic independently using their
default keyring entries or the supported environment/`.env` sources. Never put
credentials on a command line. Valid-case lookup uses `.env`, then ENV, then
keyring. Invalid-ENV cases execute outside git in a fresh directory with no
higher-priority `.env`; their generated invalid credential must not appear in
output or logs.

The owner selected the XHTML structure from atm-core's integration step panel.
`templates/integration-report/platform.xhtml.j2` retains its XML prolog, XHTML 1.0
Strict DOCTYPE, namespace, self-closed elements, facts table and CSS. The .xhtml.j2
extension selects XML escaping; consumer text is never marked safe. The template
receives the run identity, artifact and per-case results plus the overall status
and generation timestamp. CI renders it and parses the result as XML. Missing
credentials produce SKIP, which cannot pass the publication gate.

```sh
python3 scripts/integration/run.py \
  --run-id macos-run-1 --run-dir /tmp/sc-agent-integration/macos-run-1 \
  --tag prerelease/v0.1.1 --platform macos-aarch64
```

The required cases and their provider, model, operation, client and artifact
metadata have one source in `crates/decidr/integration-cases.json`. Rust derives
the wheel validator's required IDs at build time, Python reads the same records,
and `python3 scripts/integration/generate_formula.py` regenerates the poured
workflow formula. The process pours `formulas/integration.formula.toml` through
sc-compose, verifies and installs the assets, executes each case, and records
only sanitized results.
Each successful case closes its formula bead. FAIL/SKIP cases remain open with
the reason. A run directory retains the formula receipt, artifact provenance and
case results for interrupted-run recovery. Repeating the command executes only
cases without results. Use a new run ID to rerun a failed or skipped case.

`--prepare-only`, `--case <case-id>` and `--report-only` support executing individual
formula steps. The report step requires every case to have a result. It renders
XHTML with sc-compose, checks the XHTML namespace and well-formedness, and commits
and pushes `site/reports/integration/<run-id>.html` and `<run-id>.json` together.
The report records failed/skipped cases too; they never become passing evidence.
The formula root closes only when every required case passed.

Colima runs the same process inside a preconfigured Linux checkout:

```sh
python3 scripts/integration/remote.py --transport colima \
  --profile default --checkout /work/sc-agent --python python3 -- \
  --run-id linux-run-1 --run-dir /tmp/sc-agent-integration/linux-run-1 \
  --tag prerelease/v0.1.1 --platform linux-aarch64
```

For Windows, configure an SSH host with noninteractive authentication, an attached
source checkout, Python, git/gh/sc-compose/bd and that host's provider credentials:

```sh
python3 scripts/integration/remote.py --transport windows-ssh \
  --host windows-builder --checkout 'D:\work\sc-agent' --python python -- \
  --run-id windows-run-1 --run-dir 'D:\integration\windows-run-1' \
  --tag prerelease/v0.1.1 --platform windows-x86_64
```

The SSH transport uses `BatchMode=yes` and an encoded PowerShell command with
literal arguments. Transport failures cannot count as a passed platform.

## Publication preflight

`release/publish-preflight.toml` registers the live-provider check through the
sc-publish consumer seam. Run it independently with:

```sh
python3 scripts/integration/preflight.py --candidate HEAD
```

Preflight installs a checksum-verified host wheel from the recorded prerelease to
run the Rust validator; the validation host can differ from the tested platform.
It requires every live case to PASS and verifies the remote prerelease tag's
source, the downloaded target archive and both tested wheels, every case's
artifact binding, and the committed/pushed XHTML. Results and XHTML must be
committed together and pushed to origin. The tested source must be an ancestor of
the candidate; only changes under `site/reports/integration/` are accepted after
it. A source, template, script, or release-input change requires a fresh run.
Missing, skipped, failed, stale, malformed and mismatched evidence fails closed.
At least one complete passing platform is sufficient.

## Validation

```sh
cargo test --workspace --all-features
cargo fmt --check --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
python3 -m unittest discover -s scripts/integration/tests -v
SC_LINT=/path/to/sc-lint-0.8.9 python3 .github/scripts/tests/test_runtime_panic_gate.py
python3 -m pytest crates/sc-agent-py/tests crates/sc-treesitter-py/tests
```

Build and install both wheels into a Python 3.11 environment and install
`sc-observability==1.5.0` before running the Python suites. Formula/gate/transport tests use controlled inputs and temporary git
histories; their fixture PASS records are parser/composition tests and are never
live acceptance evidence. The workspace logging test `forty_thousand_sessions`
retains the NFR-SAG-0001 capacity check.

### B-1 sprint requirements and capacity evidence

The committed [B-1 sprint requirements list](b-1-sprint-requirements.md) includes
NFR-SAG-0001 and mirrors the requirement IDs on sprint bead `agt-b-1`.

Workspace test evidence: `cargo test --workspace --all-features` passed at source
commit `e9998349859efa8d4c754bf0b6ed044c91819fc8`. In `sc-agent-log`'s
`tests/session.rs`, `forty_thousand_sessions` passed; the target reported 4
passed, 0 failed, 0 ignored in 1.60 s. The test asserted 40,000 log rows and
40,000 distinct session IDs.
