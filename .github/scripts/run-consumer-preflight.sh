#!/usr/bin/env bash
# Run repository-specific checks declared in release/publish-preflight.toml.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MANIFEST="${ROOT}/release/publish-preflight.toml"

if [[ ! -f "$MANIFEST" ]]; then
  echo "run-consumer-preflight: no ${MANIFEST}; skipping" >&2
  exit 0
fi

cd "$ROOT"
python3 - "$MANIFEST" <<'PY'
import subprocess
import sys
from pathlib import Path

try:
    import tomllib
except ImportError:
    import tomli as tomllib  # type: ignore[no-redef]

manifest_path = Path(sys.argv[1]).resolve()
root = manifest_path.parent.parent
data = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
checks = data.get("checks") or []
if not checks:
    sys.exit(0)

failures: list[str] = []
for entry in checks:
    check_id = entry.get("id") or entry.get("name") or "unnamed"
    run = entry.get("run")
    if not run:
        failures.append(f"{check_id}: missing run command")
        continue
    print(f"run-consumer-preflight: running {check_id} ({run})", flush=True)
    result = subprocess.run(run, shell=True, cwd=root)
    if result.returncode != 0:
        failures.append(f"{check_id}: exit {result.returncode}")

if failures:
    for line in failures:
        print(f"run-consumer-preflight: FAIL {line}", file=sys.stderr)
    sys.exit(1)

print(f"run-consumer-preflight: OK ({len(checks)} check(s))")
PY
