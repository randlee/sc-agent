#!/usr/bin/env bash
# `cargo tree` dependency gates (docs/plans/phase-a-refine/plan.md, "`cargo
# tree` gates"). Consumer-owned: install.py never writes this path.
#
# Each row names a crate, a mode, optional extra `cargo tree` arguments and
# packages. Mode `both` runs the row twice, with default features and with
# `--no-default-features`; mode `lib` runs `--no-default-features` only,
# because a binary crate's default `cli` feature legitimately links its
# allocator and argv parser. In those modes a row fails when any package
# pattern matches. Mode `present` is the positive control: it runs with
# default features and fails when a package pattern does NOT match, so an
# absence row cannot pass just because the pattern stopped matching. Patterns are word-bounded on the
# package name and version, so `sc-jev` does not match `sc-jev-check` and
# `mimalloc` does not match `libmimalloc-sys`:
#   grep -E '(^|[^[:alnum:]_-])<name> v[0-9]'
#
# Rows added by r-4: REQ-SAG-0134 (no mimalloc in either library tree, and
# mimalloc present in each binary crate's default tree). The remaining rows of
# the plan table are added by r-3b.
#
# Feature rows (r-3c, ADR-SAG-0015, RBQA-F009) inspect
# `cargo tree -e normal,features -i <package>` instead of package names: in
# mode `lib` a row fails when any listed feature of <package> is enabled in
# the crate's `--no-default-features` tree; in mode `present` it fails when a
# listed feature is NOT enabled with default features (positive control, so
# the absence row cannot pass on a dead pattern). Pattern:
#   grep -E '<package> feature "<feature>"'
# `-e normal,features` keeps a future dev-dependency from tripping the row
# (CPR-M208). Negative control (checked when the row landed): dropping
# `default-features = false` from sc-treesitter's sc-agent-types dependency
# makes the `lib` row fail on judge, secret, obs and default.
#
# Only `cargo tree` stdout is matched; stderr (download and lock messages)
# goes to a temporary file and is printed only when cargo fails. Every call
# uses `--locked`, so Cargo.lock drift fails instead of being resolved.
#
# Usage: bash .github/scripts/cargo-tree-gates.sh
# Exit codes: 0 every row passes; 1 a row matched or `cargo tree` failed.
set -euo pipefail

cd "$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"

# crate | mode (both|lib|present) | extra cargo tree args | packages (space-separated)
rows=(
  "sc-ai|lib||mimalloc"
  "sc-treesitter|lib||mimalloc"
  "sc-ai|present||mimalloc"
  "sc-treesitter|present||mimalloc"
)

# crate | mode (lib|present) | inverted package | features (space-separated)
feature_rows=(
  "sc-treesitter|lib|sc-agent-types|judge secret obs git default"
  "sc-treesitter|present|sc-agent-types|git"
)

failures=0
errfile="$(mktemp)"
trap 'rm -f "$errfile"' EXIT

run_row() {
  local crate="$1" features="$2" extra="$3" names="$4" want="${5:-absent}"
  local -a args=(tree --locked -p "$crate" -e normal --prefix none)
  [[ -n "$features" ]] && args+=("$features")
  if [[ -n "$extra" ]]; then
    local -a extra_args
    read -r -a extra_args <<<"$extra"
    args+=("${extra_args[@]}")
  fi
  local label="cargo ${args[*]}"
  local tree
  if ! tree="$(cargo "${args[@]}" 2>"$errfile")"; then
    echo "FAIL  $label: cargo tree failed"
    sed 's/^/    /' "$errfile"
    failures=$((failures + 1))
    return
  fi
  local name hits row_failed=0
  for name in $names; do
    hits="$(printf '%s\n' "$tree" | grep -E "(^|[^[:alnum:]_-])${name} v[0-9]" || true)"
    if [[ "$want" == absent && -n "$hits" ]]; then
      echo "FAIL  $label: must not contain $name"
      printf '%s\n' "$hits" | sort -u | sed 's/^/    /'
      row_failed=1
    elif [[ "$want" == present && -z "$hits" ]]; then
      echo "FAIL  $label: must contain $name (positive control)"
      row_failed=1
    fi
  done
  if [[ "$row_failed" -eq 0 ]]; then
    echo "ok    $label: $want: $names"
  else
    failures=$((failures + 1))
  fi
}

for row in "${rows[@]}"; do
  IFS='|' read -r crate mode extra names <<<"$row"
  case "$mode" in
    both)
      run_row "$crate" "" "$extra" "$names"
      run_row "$crate" "--no-default-features" "$extra" "$names"
      ;;
    lib)
      run_row "$crate" "--no-default-features" "$extra" "$names"
      ;;
    present)
      run_row "$crate" "" "$extra" "$names" present
      ;;
    *)
      echo "cargo-tree-gates: row '$row' has unknown mode '$mode'" >&2
      exit 1
      ;;
  esac
done

run_feature_row() {
  local crate="$1" mode="$2" package="$3" features="$4"
  local -a args=(tree --locked -p "$crate" -e normal,features -i "$package" --prefix none)
  local want=absent
  case "$mode" in
    lib) args+=(--no-default-features) ;;
    present) want=present ;;
    *)
      echo "cargo-tree-gates: feature row '$crate|$mode' has unknown mode" >&2
      exit 1
      ;;
  esac
  local label="cargo ${args[*]}"
  local tree
  if ! tree="$(cargo "${args[@]}" 2>"$errfile")"; then
    echo "FAIL  $label: cargo tree failed"
    sed 's/^/    /' "$errfile"
    failures=$((failures + 1))
    return
  fi
  local feature hits row_failed=0
  for feature in $features; do
    hits="$(printf '%s\n' "$tree" | grep -E "${package} feature \"${feature}\"" || true)"
    if [[ "$want" == absent && -n "$hits" ]]; then
      echo "FAIL  $label: must not enable $package feature $feature"
      row_failed=1
    elif [[ "$want" == present && -z "$hits" ]]; then
      echo "FAIL  $label: must enable $package feature $feature (positive control)"
      row_failed=1
    fi
  done
  if [[ "$row_failed" -eq 0 ]]; then
    echo "ok    $label: $want: $package features $features"
  else
    failures=$((failures + 1))
  fi
}

for row in "${feature_rows[@]}"; do
  IFS='|' read -r crate mode package features <<<"$row"
  run_feature_row "$crate" "$mode" "$package" "$features"
done

if [[ "$failures" -ne 0 ]]; then
  echo "cargo-tree-gates: $failures row run(s) failed" >&2
  exit 1
fi
echo "cargo-tree-gates: all rows passed"
