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
LIMIT = 2580
PACKAGES = {"sc-agent", "decidr", "sc-agent-py", "sc-treesitter", "sitr"}
COMMAND_TIMEOUT_SECONDS = 900
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
    try:
        result = subprocess.run(command, cwd=ROOT, check=False, timeout=COMMAND_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        check(name, [f"timed out after {COMMAND_TIMEOUT_SECONDS} seconds"])
        return False
    check(name, [] if result.returncode == 0 else [f"exit {result.returncode}"])
    return result.returncode == 0


def rust_line_counts(text):
    """Count physical source/test lines, masking Rust literals and comments before finding test modules."""
    masked = list(text)
    tokens = re.compile(r'''//[^\n]*|/\*|(?:b|c)?r(?P<hash>\#{0,255})".*?"(?P=hash)|"(?:\\.|[^"\\])*"|b?'(?:\\.|[^'\\\n])' ''', re.S | re.X)
    offset = 0
    while match := tokens.search(text, offset):
        start, end = match.span()
        if match.group() == "/*":
            depth = 1
            for marker in re.finditer(r"/\*|\*/", text[end:]):
                depth += 1 if marker.group() == "/*" else -1
                if depth == 0:
                    end += marker.end()
                    break
            else:
                end = len(text)
        masked[start:end] = ["\n" if char == "\n" else " " for char in text[start:end]]
        offset = end
    code = "".join(masked)
    test_lines = set()
    module = re.compile(r"(?m)^[ \t]*#\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:#\[[^\n]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{")
    for match in module.finditer(code):
        depth = 1
        for brace in re.finditer(r"[{}]", code[match.end():]):
            depth += 1 if brace.group() == "{" else -1
            if depth == 0:
                end = match.end() + brace.end()
                test_lines.update(range(text.count("\n", 0, match.start()), text.count("\n", 0, end) + 1))
                break
    return len(text.splitlines()) - len(test_lines), len(test_lines)


rust = sorted(p for p in SOURCE.rglob("*.rs") if not set(p.relative_to(SOURCE).parts) & {"target", ".git"})
source_lines = test_lines = 0
for path in rust:
    parts = path.relative_to(SOURCE).parts
    text = path.read_text()
    if len(parts) > 3 and parts[0] == "crates" and parts[2] == "tests":
        test_lines += len(text.splitlines())
    else:
        source, tests = rust_line_counts(text)
        source_lines += source
        test_lines += tests
check("size", [f"{source_lines} non-test Rust lines exceeds {LIMIT}"] if source_lines > LIMIT else
      ([] if rust else ["replacement Rust source does not exist"]))
print(f"Rust source lines: {source_lines}/{LIMIT}; test lines: {test_lines}", flush=True)
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
    try:
        metadata = subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps",
                                   "--manifest-path", str(MANIFEST)], cwd=ROOT, text=True,
                                  stdout=subprocess.PIPE, check=False, timeout=COMMAND_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        check("dependencies", [f"cargo metadata timed out after {COMMAND_TIMEOUT_SECONDS} seconds"])
    else:
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
        run("rendered wheel loopback contracts", [sys.executable, "crates/sc-agent-py/tests/loopback.py"])
    else:
        check("wheel and CLI contracts", ["not run: current binaries and wheel must build first"])
        check("rendered wheel loopback contracts", ["not run: current wheel must build first"])
print("FAILED: " + ", ".join(failed) if failed else "All rewrite gates passed", flush=True)
sys.exit(bool(failed))
