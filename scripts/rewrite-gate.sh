#!/usr/bin/env bash
# Check the rewrite candidate; after old crates are removed, count the whole repository.
set -uo pipefail
root=$(git rev-parse --show-toplevel) || exit 1
cd "$root" || exit 1
status=0
manifest=rewrite/Cargo.toml
if [[ -d crates/sc-agent-http || -d crates/sc-decision-runner ]]; then
    scope=rewrite
    printf '%s\n' 'INTERIM: only rewrite/ is counted; old Rust still exists and the repository is not under the limit.'
else
    scope=.
    printf '%s\n' 'FINAL: all repository Rust files are counted.'
fi
if [[ -f $manifest ]]; then
    cargo fmt --manifest-path "$manifest" --all || status=1
else
    printf 'FAIL: %s is missing\n' "$manifest"
    status=1
fi
python3 - "$scope" <<'PY'
import os, pathlib, sys
scope = pathlib.Path(sys.argv[1])
files = []
if scope.is_dir():
    for base, dirs, names in os.walk(scope):
        dirs[:] = [name for name in dirs if name not in {'.git', 'target', '__pycache__'}]
        files.extend(pathlib.Path(base) / name for name in names if name.endswith('.rs'))
count = sum(sum(1 for _ in path.open(encoding='utf-8')) for path in files)
print(f'Rust lines ({scope}): {count}; limit: 1999; files: {len(files)}')
if count >= 2000:
    print('FAIL: Rust line limit exceeded')
    sys.exit(1)
PY
[[ $? -eq 0 ]] || status=1
if [[ -f $manifest ]]; then
    cargo metadata --manifest-path "$manifest" --no-deps --format-version 1 | python3 -c '
import json, sys
allowed = {"ureq", "serde", "serde_json", "keyring", "dotenvy", "ulid", "sc-observability", "clap", "pyo3", "tree-sitter", "thiserror", "sc-config", "ctrlc", "sc-sha"}
data = json.load(sys.stdin)
local = {p["name"] for p in data["packages"] if p["id"] in data["workspace_members"]}
bad = sorted({d["name"] for p in data["packages"] if p["id"] in data["workspace_members"] for d in p["dependencies"] if d["name"] not in allowed | local and not d["name"].startswith("tree-sitter-")})
print("Direct dependencies:", "PASS" if not bad else "FAIL: " + ", ".join(bad))
sys.exit(bool(bad))' || status=1
    cargo clippy --manifest-path "$manifest" --workspace --all-features --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::indexing_slicing || status=1
else
    printf '%s\n' 'FAIL: dependency and clippy checks cannot run without the candidate workspace.'
    status=1
fi
python3 - "$scope" <<'PY'
import ast, pathlib, re, sys, tomllib
root = pathlib.Path(sys.argv[1])
bad = []
if root.is_dir():
    for path in root.rglob('*.rs'):
        if 'target' in path.parts: continue
        text = path.read_text(encoding='utf-8')
        for name in ('Transport', 'Clock', 'CancelToken', 'Catalog', 'ReleaseDate'):
            if re.search(r'\b' + name + r'\b', text): bad.append(f'{path}: banned {name}')
        # Allow imports and the default logger function, not unrelated functions
        # merely sharing a module with it.
        clean = re.sub(r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"', lambda m: ' ' * len(m.group()), text)
        allowed = [m.span() for m in re.finditer(r'\buse\b[^;]*;', clean)]
        for m in re.finditer(r'\bfn\s+open_default_logger\b[^{]*\{', clean):
            depth, end = 1, m.end()
            while depth and end < len(clean):
                depth += (clean[end] == '{') - (clean[end] == '}')
                end += 1
            allowed.append((m.start(), end))
        for m in re.finditer(r'\bLoggerBuilder\b', clean):
            if not any(start <= m.start() < end for start, end in allowed):
                bad.append(f'{path}: LoggerBuilder outside default logger function')
        if re.search(r'fn\s+\w+\s*\([^)]*\)\s*->\s*(?:PyResult|PyErr)\b', text, re.S):
            bad.append(f'{path}: wheel function returns PyResult/PyErr')
    for path in root.rglob('pyproject.toml'):
        if 'target' in path.parts: continue
        project = tomllib.loads(path.read_text(encoding='utf-8')).get('project', {})
        if project.get('name') in ('sc-decidr', 'sitr') and any(re.match(r'sc[-_]observability(?:[<=>;\[ ]|$)', dep, re.I) for dep in project.get('dependencies', [])):
            bad.append(f'{path}: Python sc-observability dependency')
    for path in root.rglob('*.py'):
        if any(part in {'target', 'test', 'tests', '.venv', '.git'} for part in path.parts): continue
        if not any(part in {'sc_agent', 'sc_treesitter'} for part in path.parts): continue
        tree = ast.parse(path.read_text(encoding='utf-8'), filename=str(path))
        if any(isinstance(node, ast.Raise) for node in ast.walk(tree)): bad.append(f'{path}: Python raise')
print('Selected identifier and Python exception checks:', 'FAIL' if bad else 'PASS')
for item in bad: print(item)
sys.exit(bool(bad))
PY
[[ $? -eq 0 ]] || status=1
if [[ -f $manifest ]]; then
    # Build and test the candidate wheels in a fresh environment so an installed release
    # cannot accidentally satisfy these checks.
    testenv=$(mktemp -d) || exit 1
    trap 'rm -rf "$testenv"' EXIT
    if cargo build --manifest-path "$manifest" --workspace \
        && python3 -m venv "$testenv/venv" \
        && maturin build -m rewrite/crates/sc-agent-py/Cargo.toml -o "$testenv/wheels" \
        && maturin build -m rewrite/crates/sc-treesitter-py/Cargo.toml -o "$testenv/wheels" \
        && "$testenv/venv/bin/python" -m pip install --no-deps "$testenv"/wheels/*.whl; then
        testpython="$testenv/venv/bin/python"
    else
        printf '%s\n' 'FAIL: candidate binaries or wheels could not be built and installed.'
        status=1
        testpython=python3
    fi
else
    testpython=python3
fi
"$testpython" -m unittest discover -s tests/rewrite -p 'test_*.py' -v || status=1
exit "$status"
