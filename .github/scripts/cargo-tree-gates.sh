#!/usr/bin/env bash
# `cargo tree` dependency gates (docs/plans/phase-a-refine/plan.md, "`cargo
# tree` gates"). Consumer-owned: install.py never writes this path.
#
# Each row names a crate, a mode, optional extra `cargo tree` arguments and
# the packages that must be absent from its tree. Mode `both` runs the row
# twice, with default features and with `--no-default-features`; mode `lib`
# runs `--no-default-features` only, because a binary crate's default `cli`
# feature legitimately links its allocator and argv parser. A row fails when
# any absent-package pattern matches. Patterns are word-bounded on the
# package name and version, so `sc-jev` does not match `sc-jev-check` and
# `mimalloc` does not match `libmimalloc-sys`:
#   grep -E '(^|[^[:alnum:]_-])<name> v[0-9]'
#
# Rows added by r-4: REQ-SAG-0134 (no mimalloc in either library tree). The
# remaining rows of the plan table are added by r-3b.
#
# Usage: bash .github/scripts/cargo-tree-gates.sh
# Exit codes: 0 every row passes; 1 a row matched or `cargo tree` failed.
set -euo pipefail

cd "$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"

# crate | mode (both|lib) | extra cargo tree args | absent packages (space-separated)
rows=(
  "sc-ai|lib||mimalloc"
  "sc-treesitter|lib||mimalloc"
)

failures=0

run_row() {
  local crate="$1" features="$2" extra="$3" names="$4"
  local -a args=(tree -p "$crate" -e normal --prefix none)
  [[ -n "$features" ]] && args+=("$features")
  if [[ -n "$extra" ]]; then
    local -a extra_args
    read -r -a extra_args <<<"$extra"
    args+=("${extra_args[@]}")
  fi
  local label="cargo ${args[*]}"
  local tree
  if ! tree="$(cargo "${args[@]}" 2>&1)"; then
    echo "FAIL  $label: cargo tree failed"
    printf '%s\n' "$tree" | sed 's/^/    /'
    failures=$((failures + 1))
    return
  fi
  local name hits row_failed=0
  for name in $names; do
    hits="$(printf '%s\n' "$tree" | grep -E "(^|[^[:alnum:]_-])${name} v[0-9]" || true)"
    if [[ -n "$hits" ]]; then
      echo "FAIL  $label: must not contain $name"
      printf '%s\n' "$hits" | sort -u | sed 's/^/    /'
      row_failed=1
    fi
  done
  if [[ "$row_failed" -eq 0 ]]; then
    echo "ok    $label: absent: $names"
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
    *)
      echo "cargo-tree-gates: row '$row' has unknown mode '$mode'" >&2
      exit 1
      ;;
  esac
done

if [[ "$failures" -ne 0 ]]; then
  echo "cargo-tree-gates: $failures row run(s) failed" >&2
  exit 1
fi
echo "cargo-tree-gates: all rows passed"
