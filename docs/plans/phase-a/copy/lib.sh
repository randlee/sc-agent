#!/usr/bin/env bash
# Shared helpers for the phase-a copy scripts. Source it from a sprint script:
#   . "$(dirname "$0")/lib.sh"
# Environment:
#   SC_AGENT_REF  path to a clone of randlee/sc-agent-ref (default: ../sc-agent-ref next to this repo)
#   REF_COMMIT    source commit (default: dce239a, the latest code)
#   TARGET        directory files are copied into (default: this repository's root)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../../.." && pwd)"
SC_AGENT_REF="${SC_AGENT_REF:-$REPO_ROOT/../sc-agent-ref}"
REF_COMMIT="${REF_COMMIT:-dce239a}"
TARGET="${TARGET:-$REPO_ROOT}"

git -C "$SC_AGENT_REF" cat-file -e "$REF_COMMIT^{commit}" 2>/dev/null || {
  echo "copy: $SC_AGENT_REF does not have $REF_COMMIT; run: git -C \"$SC_AGENT_REF\" fetch origin '+refs/heads/*:refs/remotes/origin/*'" >&2
  exit 2
}

# copy_file <source path in sc-agent-ref> <destination path under TARGET>
copy_file() {
  local src="$1" dst="$TARGET/$2"
  mkdir -p "$(dirname "$dst")"
  git -C "$SC_AGENT_REF" show "$REF_COMMIT:$src" > "$dst"
  echo "copied $src -> $2"
}

# copy_lines <source path> <first line> <last line> <destination path>   (appends)
copy_lines() {
  local src="$1" first="$2" last="$3" dst="$TARGET/$4"
  mkdir -p "$(dirname "$dst")"
  git -C "$SC_AGENT_REF" show "$REF_COMMIT:$src" | sed -n "${first},${last}p" >> "$dst"
  echo "copied $src:$first-$last -> $4"
}
