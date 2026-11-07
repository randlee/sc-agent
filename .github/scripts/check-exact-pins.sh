#!/usr/bin/env bash
# Exact-pin check (NFR-SAG-0004, ADR-SAG-0023). Consumer-owned: install.py
# never writes this path.
#
# Every `=` version requirement in any Cargo.toml must name a crate on the
# ADR-SAG-0023 lockstep list. The list has exactly one copy: the lines
# between `<!-- lockstep-pins:start -->` and `<!-- lockstep-pins:end -->` in
# docs/architecture.md. This script reads it from there and keeps no list of
# its own.
#
# Usage:
#   bash .github/scripts/check-exact-pins.sh              check the repository
#   bash .github/scripts/check-exact-pins.sh --self-test  prove the check fails
#                                                        on an unlisted `=`
#   bash .github/scripts/check-exact-pins.sh --root DIR   check another tree
#
# Scanned: every Cargo.toml under the root, skipping `target` and hidden
# directories. Tables: [workspace.dependencies], [dependencies],
# [dev-dependencies], [build-dependencies] and their [target.*] forms. A
# renamed dependency (`package = "..."`) is checked under its package name.
# A requirement is exact when any comma-separated comparator starts with `=`
# (`>=` and `<=` are ranges, not pins).
#
# Exit codes: 0 every pin is listed; 1 an unlisted pin, or a missing or empty
# list; 2 usage error. Needs python3 (3.11+, for tomllib).
set -euo pipefail

script_path="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"

check_tree() {
  local root="$1"
  python3 -I - "$root" <<'PY'
import os
import sys
import tomllib

root = sys.argv[1]
start_marker = "<!-- lockstep-pins:start -->"
end_marker = "<!-- lockstep-pins:end -->"
adr_path = os.path.join(root, "docs", "architecture.md")

try:
    with open(adr_path, encoding="utf-8") as handle:
        lines = [line.strip() for line in handle]
except OSError as err:
    sys.exit(f"check-exact-pins: cannot read {adr_path}: {err}")

if lines.count(start_marker) != 1 or lines.count(end_marker) != 1:
    sys.exit(
        f"check-exact-pins: {adr_path} must contain {start_marker} and "
        f"{end_marker} exactly once each"
    )
begin = lines.index(start_marker)
end = lines.index(end_marker)
if end < begin:
    sys.exit(f"check-exact-pins: {end_marker} precedes {start_marker}")
allowed = {line for line in lines[begin + 1 : end] if line}
if not allowed:
    sys.exit("check-exact-pins: the ADR-SAG-0023 lockstep list is empty")

DEP_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def exact(requirement):
    return any(part.strip().startswith("=") for part in requirement.split(","))


def dependency_tables(document):
    workspace = document.get("workspace", {})
    if isinstance(workspace.get("dependencies"), dict):
        yield "workspace.dependencies", workspace["dependencies"]
    for table in DEP_TABLES:
        if isinstance(document.get(table), dict):
            yield table, document[table]
    for target, body in document.get("target", {}).items():
        for table in DEP_TABLES:
            if isinstance(body.get(table), dict):
                yield f"target.{target}.{table}", body[table]


manifests = []
for directory, subdirs, files in os.walk(root):
    subdirs[:] = sorted(d for d in subdirs if d != "target" and not d.startswith("."))
    if "Cargo.toml" in files:
        manifests.append(os.path.join(directory, "Cargo.toml"))

pins = []
for manifest in sorted(manifests):
    with open(manifest, "rb") as handle:
        try:
            document = tomllib.load(handle)
        except tomllib.TOMLDecodeError as err:
            sys.exit(f"check-exact-pins: {manifest}: {err}")
    relative = os.path.relpath(manifest, root)
    for table, deps in dependency_tables(document):
        for key, spec in deps.items():
            if isinstance(spec, str):
                name, requirement = key, spec
            elif isinstance(spec, dict) and isinstance(spec.get("version"), str):
                name, requirement = spec.get("package", key), spec["version"]
            else:
                continue
            if exact(requirement):
                pins.append((relative, table, name, requirement))

unlisted = [pin for pin in pins if pin[2] not in allowed]
for relative, table, name, requirement in pins:
    verdict = "ok" if name in allowed else "NOT LISTED"
    print(f"{verdict:>10}  {relative} [{table}] {name} = \"{requirement}\"")
if unlisted:
    print(
        f"check-exact-pins: {len(unlisted)} exact pin(s) not on the ADR-SAG-0023 "
        "lockstep list (docs/architecture.md); use a caret range or add the crate "
        "to the list with its reason",
        file=sys.stderr,
    )
    sys.exit(1)
print(f"check-exact-pins: {len(pins)} exact pin(s), all on the lockstep list")
PY
}

self_test() {
  local work
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' RETURN
  local failures=0

  mkdir -p "$work/docs" "$work/crates/lib"
  cat >"$work/docs/architecture.md" <<'MD'
  <!-- lockstep-pins:start -->
  listed-crate
  <!-- lockstep-pins:end -->
MD
  cat >"$work/Cargo.toml" <<'TOML'
[workspace.dependencies]
listed-crate = "=1.0.0"
caret-crate = "1"
range-crate = ">=1, <2"
TOML
  cat >"$work/crates/lib/Cargo.toml" <<'TOML'
[package]
name = "lib"
[dependencies]
listed-crate = { workspace = true }
renamed = { package = "listed-crate", version = "=1.0.0" }
TOML

  expect() {
    local want="$1" label="$2" got=0
    check_tree "$work" >"$work/out.txt" 2>&1 || got=$?
    if [[ "$want" == pass && "$got" -eq 0 ]] || [[ "$want" == fail && "$got" -ne 0 ]]; then
      echo "self-test ok:   $label (exit $got)"
    else
      echo "self-test FAIL: $label (expected $want, exit $got)"
      sed 's/^/    /' "$work/out.txt"
      failures=$((failures + 1))
    fi
  }

  expect pass "listed pins, caret and range requirements"

  printf '[dev-dependencies]\nunlisted-crate = "=0.1.0"\n' >>"$work/crates/lib/Cargo.toml"
  expect fail "injected unlisted = in [dev-dependencies]"

  cat >"$work/crates/lib/Cargo.toml" <<'TOML'
[package]
name = "lib"
[target.'cfg(unix)'.dependencies]
alias = { package = "unlisted-crate", version = "= 0.1.0" }
TOML
  expect fail "injected unlisted = behind a rename in a [target] table"

  printf '[package]\nname = "lib"\n' >"$work/crates/lib/Cargo.toml"
  printf '# no markers\n' >"$work/docs/architecture.md"
  expect fail "lockstep list markers missing"

  if [[ "$failures" -ne 0 ]]; then
    echo "check-exact-pins self-test: $failures case(s) failed" >&2
    return 1
  fi
  echo "check-exact-pins self-test: all cases passed"
}

case "${1:-}" in
  "")
    check_tree "$(git -C "$(dirname "$script_path")" rev-parse --show-toplevel)"
    ;;
  --root)
    [[ $# -eq 2 ]] || { echo "usage: $0 [--self-test | --root DIR]" >&2; exit 2; }
    check_tree "$2"
    ;;
  --self-test)
    [[ $# -eq 1 ]] || { echo "usage: $0 [--self-test | --root DIR]" >&2; exit 2; }
    self_test
    ;;
  *)
    echo "usage: $0 [--self-test | --root DIR]" >&2
    exit 2
    ;;
esac
