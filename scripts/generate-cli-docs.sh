#!/usr/bin/env bash
# Regenerate fixtures/cli/*.json and site/cli/{decidr,sitr}/ from the clap trees.
# Uses the host `docli` binary pinned in release/docli-pin.toml (sc-docli 0.2.0).
# One `docli generate` writes a single index.html, so each CLI gets its own directory
# under the standard site/cli/ tree.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PIN="${ROOT}/release/docli-pin.toml"
VERSION="$(
  python3 - "$PIN" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8")
match = re.search(r'(?m)^sc_docli_version\s*=\s*"([^"]+)"\s*$', text)
if not match:
    sys.stderr.write(f"generate-cli-docs: no sc_docli_version in {sys.argv[1]}\n")
    sys.exit(1)
print(match.group(1))
PY
)"

installed="$(docli --version 2>/dev/null || true)"
if [[ "${installed}" != "docli ${VERSION}" ]]; then
  cargo install sc-docli --version "${VERSION}" --locked
fi

mkdir -p fixtures/cli site/cli/decidr site/cli/sitr

cargo run --locked --quiet --example gen-cli-model --package decidr -- \
  --output fixtures/cli/decidr-cli-model.json
cargo run --locked --quiet --example gen-cli-model --package sitr -- \
  --output fixtures/cli/sitr-cli-model.json

docli generate --input fixtures/cli/decidr-cli-model.json --html site/cli/decidr
docli generate --input fixtures/cli/sitr-cli-model.json --html site/cli/sitr

python3 - <<'PY'
from pathlib import Path

checks = {
    Path("site/cli/decidr/index.html"): ("decidr send", "decidr doctor"),
    Path("site/cli/sitr/index.html"): ("sitr query", "sitr languages"),
}
for path, needles in checks.items():
    text = path.read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    if missing:
        raise SystemExit(f"generate-cli-docs: {path} missing {missing}")
PY
