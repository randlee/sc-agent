#!/usr/bin/env python3
"""Build and verify the shipped five-crate workspace and installed wheel."""
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT
MANIFEST = SOURCE / "Cargo.toml"
ALLOWED = set("ureq serde serde_json keyring dotenvy ulid sc-sha sc-config ctrlc sc-observability clap pyo3 thiserror tree-sitter tree-sitter-python tree-sitter-rust".split())
PACKAGES = {"sc-agent", "decidr", "sc-agent-py", "sc-treesitter", "sitr"}
failed = []


def check(name, failures):
    problems = list(failures)
    print(f"{'FAIL' if problems else 'PASS'} {name}", flush=True)
    for problem in problems:
        print(f"  {problem}", flush=True)
    if problems:
        failed.append(name)


def run(name, command):
    print("+ " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=ROOT, check=False)
    check(name, [] if result.returncode == 0 else [f"exit {result.returncode}"])
    return result.returncode == 0


rust = sorted(p for p in SOURCE.rglob("*.rs") if not set(p.relative_to(SOURCE).parts) & {"target", ".git"})
lines = sum(len(p.read_text().splitlines()) for p in rust)
check("size", [f"{lines} Rust lines exceeds 2200"] if lines > 2200 else
      ([] if rust else ["replacement Rust source does not exist"]))
print(f"Rust lines: {lines}/2200 across {len(rust)} files", flush=True)
check("forbidden architecture", (
    f"{p.relative_to(ROOT)}:{i}: {match.group()}"
    for p in rust for i, line in enumerate(p.read_text().splitlines(), 1)
    for match in re.finditer(r"\b(?:Transport|Clock|CancelToken|Catalog)\b|\btrait\s+|\bpub\s+use\b", line)
))
# The only permitted builder use is in the default_logger function.
check("logger builder", (
    str(p.relative_to(ROOT)) for p in rust
    if "LoggerBuilder" in re.sub(r"(?ms)^pub fn default_logger\b.*?^}", "", p.read_text())
))
check("Python dependency", (
    str(p.relative_to(ROOT)) for p in SOURCE.rglob("pyproject.toml")
    if any(re.match(r"sc[-_]observability\b", dep) for dep in
           tomllib.loads(p.read_text()).get("project", {}).get("dependencies", []))
))
python = [p for p in (ROOT / "crates/sc-agent-py/python").rglob("*.py") if "tests" not in p.parts and "target" not in p.parts]
templates = list(SOURCE.rglob("consumer.py.j2"))
check("returned errors", (
    str(p.relative_to(ROOT)) for p in python + templates
    if re.search(r"(?m)^\s*raise\b", p.read_text())
))
check("wheel signatures", (
    str(p.relative_to(ROOT)) for p in rust if "sc-agent-py" in p.parts
    and re.search(r"(?s)#\[pyfunction[^]]*\]\s*fn\s+\w+[^{}]*->\s*(?:PyResult|Result\s*<[^;{}]*PyErr)", p.read_text())
))
cli_ready = False
if MANIFEST.exists():
    metadata = subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps",
                               "--manifest-path", str(MANIFEST)], cwd=ROOT, text=True,
                              stdout=subprocess.PIPE, check=False)
    if metadata.returncode:
        check("dependencies", [f"cargo metadata exit {metadata.returncode}"])
    else:
        packages = json.loads(metadata.stdout)["packages"]
        local = {p["name"] for p in packages}
        check("workspace members", sorted(local - PACKAGES))
        check("dependencies", (
            f"{p['name']}: {d['name']}" for p in packages for d in p["dependencies"]
            if d["name"] not in ALLOWED | local
        ))
    run("format", ["cargo", "fmt", "--check", "--all", "--manifest-path", str(MANIFEST)])
    run("clippy", ["cargo", "clippy", "--locked", "--workspace", "--all-features", "--manifest-path", str(MANIFEST),
                   "--", "-D", "warnings", "-D", "clippy::unwrap_used", "-D", "clippy::expect_used",
                   "-D", "clippy::panic", "-D", "clippy::indexing_slicing"])
    run("Rust contracts", ["cargo", "test", "--locked", "--workspace", "--all-features", "--manifest-path", str(MANIFEST)])
    cli_ready = run("CLI build", ["cargo", "build", "--locked", "--workspace", "--bins", "--manifest-path", str(MANIFEST)])
else:
    check("workspace", [f"missing {MANIFEST.relative_to(ROOT)}"])
with tempfile.TemporaryDirectory(prefix="sc-agent-rewrite-") as temporary:
    wheel_manifest = SOURCE / "crates/sc-agent-py/Cargo.toml"
    installed = False
    if run("wheel build", ["maturin", "build", "--locked", "--manifest-path", str(wheel_manifest), "--out", temporary]):
        wheels = list(Path(temporary).glob("*.whl"))
        if len(wheels) == 1:
            installed = run("wheel install", [sys.executable, "-m", "pip", "install", "--no-deps", "--force-reinstall", str(wheels[0])])
        else:
            check("wheel install", [f"expected one wheel, found {len(wheels)}"])
    if installed and cli_ready:
        run("wheel and CLI contracts", [sys.executable, "-m", "unittest", "discover", "-s", "tests/contract", "-v"])
    else:
        check("wheel and CLI contracts", ["not run: current binaries and wheel must build first"])
print("FAILED: " + ", ".join(failed) if failed else "All rewrite gates passed", flush=True)
sys.exit(bool(failed))
