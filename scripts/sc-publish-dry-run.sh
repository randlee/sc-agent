#!/usr/bin/env bash
# Release dry-run for sc-agent (REQ-SAG-0132, REQ-SAG-0803).
#
# One recipe for local use and PR CI (.github/workflows/sc-publish-dry-run.yml):
#   kit      1. clone the sc-publish kit at the pinned SHA into .sc-publish-kit/
#            2. fail on kit drift (install.py --dry-run must exit 0, ADR-SAG-0018)
#   checks   3. validate the rendered manifest and Cargo version lockstep
#            4. run the kit's consumer preflight runner
#   publish  5. cargo publish --dry-run --locked for every manifest crate, in order
#   wheel    6. maturin build the sc-jev-py wheel
#
# Usage: bash scripts/sc-publish-dry-run.sh [all|kit|checks|publish|wheel]...
# With no argument every phase runs in order. CI runs one phase per step so
# each step has its own timeout.
#
# This script never tags, uploads, or publishes. Environment:
#   SC_PUBLISH_ALLOW_DIRTY=1  pass --allow-dirty to cargo publish
#   SC_AGENT_WHEEL_DIR=<dir>  keep the wheel in <dir>. The directory must be
#                             missing, empty, or named `sc-agent-wheels`; it is
#                             cleared first. Unset: a mktemp dir removed on exit.
#   SC_PUBLISH_NET_TIMEOUT    seconds for each git / bootstrap network step (300)
#   SC_PUBLISH_BUILD_TIMEOUT  seconds for cargo publish and maturin (1800)
# The publish phase builds in target/sc-publish-dry-run/<content key> (see
# phase_publish). One directory accumulates per distinct source state and
# this script does not prune them. CI keeps them out of its target/ cache
# (sc-publish-dry-run.yml); locally `cargo clean` or removing
# target/sc-publish-dry-run/ reclaims the space.
# Network settings default to CARGO_HTTP_TIMEOUT=60, CARGO_NET_RETRY=2 and
# PIP_DEFAULT_TIMEOUT=60. A timed-out step fails and names itself; this script
# never retries.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

pin_file="release/sc-publish-pin.toml"
consumer_input="release/sc-publish-consumer-input.json"
manifest="release/publish-artifacts.toml"
kit_dir=".sc-publish-kit"
venv_dir=".sc-publish-venv"
net_timeout="${SC_PUBLISH_NET_TIMEOUT:-300}"
build_timeout="${SC_PUBLISH_BUILD_TIMEOUT:-1800}"

export CARGO_HTTP_TIMEOUT="${CARGO_HTTP_TIMEOUT:-60}"
export CARGO_NET_RETRY="${CARGO_NET_RETRY:-2}"
export PIP_DEFAULT_TIMEOUT="${PIP_DEFAULT_TIMEOUT:-60}"

step() { printf '\n==> %s\n' "$*"; }
die() {
  echo "sc-publish-dry-run: $*" >&2
  exit 1
}

# bounded <seconds> <step name> <command>...
# Runs the command under a deadline: `timeout` or `gtimeout` when present,
# otherwise python3 (which the kit requires anyway).
bounded() {
  local seconds="$1" name="$2" rc=0
  shift 2
  if command -v timeout >/dev/null 2>&1; then
    timeout "$seconds" "$@" || rc=$?
  elif command -v gtimeout >/dev/null 2>&1; then
    gtimeout "$seconds" "$@" || rc=$?
  else
    python3 -c '
import subprocess, sys
try:
    sys.exit(subprocess.run(sys.argv[2:], timeout=int(sys.argv[1])).returncode)
except subprocess.TimeoutExpired:
    sys.exit(124)
' "$seconds" "$@" || rc=$?
  fi
  if [[ "$rc" -eq 124 ]]; then
    die "step '$name' timed out after ${seconds}s"
  elif [[ "$rc" -ne 0 ]]; then
    die "step '$name' failed (exit $rc)"
  fi
}

# Prints the pinned repository URL and revision, one per line, after
# validating both. tomllib needs Python 3.11+.
read_pin() {
  python3 - "$pin_file" <<'PY'
import re
import sys
import tomllib
from urllib.parse import urlparse

path = sys.argv[1]
try:
    with open(path, "rb") as handle:
        pin = tomllib.load(handle).get("sc-publish", {})
except (OSError, tomllib.TOMLDecodeError) as error:
    sys.exit(f"sc-publish-dry-run: cannot read {path}: {error}")
repository = pin.get("repository")
revision = pin.get("revision")
url = urlparse(repository) if isinstance(repository, str) else None
if url is None or url.scheme != "https" or not url.netloc:
    sys.exit(f"sc-publish-dry-run: {path} [sc-publish].repository must be an https URL")
if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
    sys.exit(f"sc-publish-dry-run: {path} must pin a full 40-character revision")
print(repository)
print(revision)
PY
}

phase_kit() {
  local pin kit_repository kit_revision publish_python
  pin="$(read_pin)"
  kit_repository="$(sed -n 1p <<<"$pin")"
  kit_revision="$(sed -n 2p <<<"$pin")"

  step "sc-publish kit at $kit_revision"
  if [[ ! -d "$kit_dir/.git" ]]; then
    bounded "$net_timeout" "git clone sc-publish kit" \
      git clone --quiet --depth 1 "$kit_repository" "$kit_dir"
  fi
  git -C "$kit_dir" remote set-url origin "$kit_repository"
  if [[ "$(git -C "$kit_dir" rev-parse HEAD)" != "$kit_revision" ]]; then
    bounded "$net_timeout" "git fetch sc-publish kit revision" \
      git -C "$kit_dir" fetch --quiet origin "$kit_revision" --depth 1
    git -C "$kit_dir" checkout --quiet --force "$kit_revision"
  fi
  # A reused .sc-publish-kit must not carry local edits or stray files.
  git -C "$kit_dir" reset --hard --quiet
  git -C "$kit_dir" clean -fdxq

  step "sc-compose bootstrap"
  publish_python="$(bounded "$net_timeout" "bootstrap sc-compose venv" \
    python3 "$kit_dir/plugins/sc-publish/.github/scripts/bootstrap_sc_compose.py" --venv "$venv_dir")"

  step "kit drift check (install.py --dry-run)"
  "$publish_python" "$kit_dir/plugins/sc-publish/install.py" --dry-run --input "$consumer_input" .
}

phase_checks() {
  step "manifest validation and version lockstep"
  python3 .github/scripts/release_artifacts.py validate-manifest --manifest "$manifest" --workspace-toml Cargo.toml
  python3 .github/scripts/release_artifacts.py verify-version-lockstep --manifest "$manifest" --workspace-toml Cargo.toml

  step "consumer preflight"
  bash .github/scripts/run-consumer-preflight.sh
}

phase_publish() {
  local publish_args=() package _wait
  step "cargo publish --dry-run (manifest publish order)"
  while IFS='|' read -r package _wait; do
    publish_args+=(-p "$package")
  done < <(python3 .github/scripts/release_artifacts.py list-publish-plan --manifest "$manifest")
  [[ "${#publish_args[@]}" -gt 0 ]] || die "$manifest lists no crates"
  if [[ "${SC_PUBLISH_ALLOW_DIRTY:-0}" == "1" ]]; then
    publish_args+=(--allow-dirty)
  fi
  # A multi-package dry-run serves the packaged workspace crates from a
  # local registry at <target-dir>/package/tmp-registry and unpacks each one
  # under $CARGO_HOME/registry/src/-<hash of that path>/. Cargo does not
  # unpack again while that directory exists, so with a fixed target dir a
  # copy from an earlier run of the same version (locally, or restored from
  # the CI cache of ~/.cargo/registry) is compiled instead of the current
  # source, and a new public item in sc-agent-types fails to resolve when
  # sc-jev-check is verified. Keying the target dir on the source content
  # means a reused unpack always holds identical sources. The key covers the
  # HEAD tree, the working diff, and untracked non-ignored files (paths and
  # contents), which `--allow-dirty` packages too.
  local content_key
  content_key="$(python3 -c '
import hashlib, subprocess
def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True).stdout
digest = hashlib.sha256(git("rev-parse", "HEAD^{tree}") + git("diff", "HEAD", "--binary"))
for name in sorted(git("ls-files", "-z", "--others", "--exclude-standard").split(b"\0")):
    if name:
        with open(name, "rb") as handle:
            digest.update(name + b"\0" + handle.read())
print(digest.hexdigest()[:16])
')"
  publish_args+=(--target-dir "target/sc-publish-dry-run/$content_key")
  printf 'cargo publish --dry-run --locked %s\n' "${publish_args[*]}"
  bounded "$build_timeout" "cargo publish --dry-run" \
    cargo publish --dry-run --locked "${publish_args[@]}"
}

# Absolute, symlink-resolved path of an existing directory.
real_dir() { (cd "$1" && pwd -P); }

# Sets wheel_dir. Unset SC_AGENT_WHEEL_DIR gets a private mktemp dir removed
# on exit; a caller-chosen dir is cleared only when that cannot hurt.
wheel_dir=""
wheel_tmp_dir=""
prepare_wheel_dir() {
  local dir="${SC_AGENT_WHEEL_DIR:-}" resolved
  if [[ -z "$dir" ]]; then
    local tmp_root="${TMPDIR:-/tmp}"
    wheel_tmp_dir="$(mktemp -d "${tmp_root%/}/sc-agent-wheels.XXXXXX")"
    trap 'rm -rf -- "$wheel_tmp_dir"' EXIT
    wheel_dir="$wheel_tmp_dir"
    return
  fi
  if [[ -e "$dir" && ! -d "$dir" ]]; then
    die "SC_AGENT_WHEEL_DIR=$dir exists and is not a directory"
  fi
  if [[ -d "$dir" && -n "$(ls -A "$dir")" ]]; then
    resolved="$(real_dir "$dir")"
    if [[ "$resolved" == "/" || "$resolved" == "$(real_dir "$repo_root")" ]]; then
      die "refusing to clear SC_AGENT_WHEEL_DIR=$dir (resolves to $resolved)"
    fi
    [[ "$(basename "$resolved")" == "sc-agent-wheels" ]] ||
      die "refusing to clear non-empty SC_AGENT_WHEEL_DIR=$dir: its basename must be 'sc-agent-wheels'"
    rm -rf -- "$resolved"
  fi
  mkdir -p "$dir"
  wheel_dir="$dir"
}

phase_wheel() {
  local wheels
  step "maturin build sc-jev-py"
  command -v maturin >/dev/null 2>&1 ||
    die "maturin is required (pip install 'maturin==1.16.0', the CI pin)"
  prepare_wheel_dir
  bounded "$build_timeout" "maturin build sc-jev-py" \
    maturin build --release -m crates/sc-jev-py/Cargo.toml -o "$wheel_dir"
  shopt -s nullglob
  wheels=("$wheel_dir"/*.whl)
  shopt -u nullglob
  [[ "${#wheels[@]}" -gt 0 ]] || die "maturin produced no wheel in $wheel_dir"
  printf 'wheel: %s\n' "${wheels[@]}"
  if [[ -n "$wheel_tmp_dir" ]]; then
    echo "(temporary; set SC_AGENT_WHEEL_DIR to keep the wheel)"
  fi
}

phases=("$@")
[[ "${#phases[@]}" -gt 0 ]] || phases=(all)
for phase in "${phases[@]}"; do
  case "$phase" in
    all) phase_kit; phase_checks; phase_publish; phase_wheel ;;
    kit) phase_kit ;;
    checks) phase_checks ;;
    publish) phase_publish ;;
    wheel) phase_wheel ;;
    *) die "unknown phase '$phase' (expected all, kit, checks, publish, wheel)" ;;
  esac
done

step "sc-publish dry-run passed (${phases[*]})"
