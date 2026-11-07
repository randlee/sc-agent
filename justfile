# Managed by sc-lint; regenerate with `sc-lint init --just`.
set windows-shell := ["cmd.exe", "/c"]

python := if os_family() == "windows" { "python" } else { "python3" }

default: lint

setup:
    {{python}} .sc-lint/bootstrap.py setup --config sc-lint.toml

lint *profile:
    {{python}} .sc-lint/bootstrap.py lint --config sc-lint.toml {{profile}}

test *layer:
    {{python}} .sc-lint/bootstrap.py test --config sc-lint.toml {{layer}}

upgrade:
    {{python}} .sc-lint/bootstrap.py upgrade --config sc-lint.toml

# Install the sc-ai binary from this workspace (REQ-SAG-0703).
install-sc-ai:
    cargo install --path crates/sc-ai --locked

# Python harness for sc-jev-py (REQ-SAG-0801, REQ-SAG-0802). Builds the
# `extension-module,test-util` extension into a venv with `maturin develop`,
# then runs tests/fixtures/jev-python pytest. This is the test build; the
# publish wheel leaves `test-util` off (ADR-SAG-0022). CI runs this recipe
# (.github/workflows/sc-agent-ci.yml, job `python`) with JEV_VENV under
# $RUNNER_TEMP. The venv is recreated (`--clear`) on every run. maturin
# 1.16.0, pytest 8.4.2 and their transitive dependencies install from the
# hash-pinned lock tests/fixtures/jev-python/constraints.txt, passed as `-r`:
# pip 24.0 (Python 3.11.9) ignores constraint-file hashes for top-level
# requirements, so `-c` would not enforce them.
jev_venv := env("JEV_VENV", justfile_directory() / "target" / "jev-python-venv")
jev_python := if os_family() == "windows" { replace(jev_venv / "Scripts" / "python.exe", "/", "\\") } else { jev_venv / "bin" / "python" }
# cmd.exe strips the first and last quote of a line that starts with one, so
# the interpreter path is quoted on Unix only. On Windows JEV_VENV (and the
# default checkout path) must not contain spaces.
jev_q := if os_family() == "windows" { "" } else { "\"" }

test-python $VIRTUAL_ENV=jev_venv:
    {{python}} -m venv --clear "{{jev_venv}}"
    {{jev_q}}{{jev_python}}{{jev_q}} -m pip install --disable-pip-version-check --timeout 60 --require-hashes -r tests/fixtures/jev-python/constraints.txt
    {{jev_q}}{{jev_python}}{{jev_q}} -m maturin develop --features extension-module,test-util -m crates/sc-jev-py/Cargo.toml
    {{jev_q}}{{jev_python}}{{jev_q}} -m pytest tests/fixtures/jev-python -q
