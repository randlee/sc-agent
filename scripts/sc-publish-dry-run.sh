#!/usr/bin/env bash
# Release dry-run for sc-agent (REQ-SAG-0132, REQ-SAG-0803).
#
# One recipe for local use and PR CI (.github/workflows/sc-publish-dry-run.yml):
#   1. clone the sc-publish kit at the pinned SHA into .sc-publish-kit/
#   2. fail on kit drift (install.py --dry-run must exit 0, ADR-SAG-0018)
#   3. validate the rendered manifest and Cargo version lockstep
#   4. run the kit's consumer preflight runner
#   5. cargo publish --dry-run --locked for every manifest crate, in order
#   6. maturin build the sc-jev-py wheel
#
# This script never tags, uploads, or publishes. Set SC_PUBLISH_ALLOW_DIRTY=1
# to pass --allow-dirty to cargo publish when the working tree has edits.
# Set SC_AGENT_WHEEL_DIR to choose the wheel output directory.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

pin_file="release/sc-publish-pin.toml"
consumer_input="release/sc-publish-consumer-input.json"
manifest="release/publish-artifacts.toml"
kit_dir=".sc-publish-kit"
venv_dir=".sc-publish-venv"
wheel_dir="${SC_AGENT_WHEEL_DIR:-${TMPDIR:-/tmp}/sc-agent-wheels}"

step() { printf '\n==> %s\n' "$*"; }

kit_repository="$(sed -n 's/^repository = "\(.*\)"/\1/p' "$pin_file")"
kit_revision="$(sed -n 's/^revision = "\([0-9a-f]*\)"/\1/p' "$pin_file")"
if [[ ! "$kit_revision" =~ ^[0-9a-f]{40}$ ]]; then
  echo "sc-publish-dry-run: $pin_file must pin a full 40-character revision" >&2
  exit 1
fi

step "sc-publish kit at $kit_revision"
if [[ ! -d "$kit_dir/.git" ]]; then
  git clone --quiet --depth 1 "$kit_repository" "$kit_dir"
fi
if [[ "$(git -C "$kit_dir" rev-parse HEAD)" != "$kit_revision" ]]; then
  git -C "$kit_dir" fetch --quiet origin "$kit_revision" --depth 1
  git -C "$kit_dir" checkout --quiet "$kit_revision"
fi
publish_python="$(python3 "$kit_dir/plugins/sc-publish/.github/scripts/bootstrap_sc_compose.py" --venv "$venv_dir")"

step "kit drift check (install.py --dry-run)"
"$publish_python" "$kit_dir/plugins/sc-publish/install.py" --dry-run --input "$consumer_input" .

step "manifest validation and version lockstep"
python3 .github/scripts/release_artifacts.py validate-manifest --manifest "$manifest" --workspace-toml Cargo.toml
python3 .github/scripts/release_artifacts.py verify-version-lockstep --manifest "$manifest" --workspace-toml Cargo.toml

step "consumer preflight"
bash .github/scripts/run-consumer-preflight.sh

step "cargo publish --dry-run (manifest publish order)"
publish_args=()
while IFS='|' read -r package _wait; do
  publish_args+=(-p "$package")
done < <(python3 .github/scripts/release_artifacts.py list-publish-plan --manifest "$manifest")
if [[ "${SC_PUBLISH_ALLOW_DIRTY:-0}" == "1" ]]; then
  publish_args+=(--allow-dirty)
fi
printf 'cargo publish --dry-run --locked %s\n' "${publish_args[*]}"
cargo publish --dry-run --locked "${publish_args[@]}"

step "maturin build sc-jev-py"
if ! command -v maturin >/dev/null 2>&1; then
  echo "sc-publish-dry-run: maturin is required (pip install 'maturin>=1.8,<2')" >&2
  exit 1
fi
rm -rf "$wheel_dir"
maturin build --release -m crates/sc-jev-py/Cargo.toml -o "$wheel_dir"
shopt -s nullglob
wheels=("$wheel_dir"/*.whl)
if [[ "${#wheels[@]}" -eq 0 ]]; then
  echo "sc-publish-dry-run: maturin produced no wheel in $wheel_dir" >&2
  exit 1
fi
printf 'wheel: %s\n' "${wheels[@]}"

step "sc-publish dry-run passed"
