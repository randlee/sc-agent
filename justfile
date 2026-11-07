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
# `extension-module,test-util` extension into a venv under target/ with
# `maturin develop`, then runs tests/fixtures/jev-python pytest. This is the
# test build; the publish wheel leaves `test-util` off (ADR-SAG-0022). CI runs
# this recipe (.github/workflows/sc-agent-ci.yml, job `python`).
jev_venv := "target/jev-python-venv"
jev_python := if os_family() == "windows" { "target\\jev-python-venv\\Scripts\\python.exe" } else { "target/jev-python-venv/bin/python" }
maturin_version := env("MATURIN_VERSION", "1.16.0")
pytest_version := env("PYTEST_VERSION", "8.4.2")

test-python $VIRTUAL_ENV=(justfile_directory() / jev_venv):
    {{python}} -m venv {{jev_venv}}
    {{jev_python}} -m pip install --disable-pip-version-check --timeout 60 "maturin=={{maturin_version}}" "pytest=={{pytest_version}}"
    {{jev_python}} -m maturin develop --features extension-module,test-util -m crates/sc-jev-py/Cargo.toml
    {{jev_python}} -m pytest tests/fixtures/jev-python -q
