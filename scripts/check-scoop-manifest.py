#!/usr/bin/env python3
"""Check that the rendered Scoop manifest shims every released binary.

Repo-owned guard (bead agt-0mi, REQ-SAG-0133). The kit's scoop-publish.yml
builds the template variables from the consumer manifest and renders
release/scoop/manifest.json.j2 with the published renderer. This script
repeats that on the pinned kit: it takes the `binary` expression from the
kit workflow, evaluates it against `[channels.scoop]` in
release/publish-artifacts.toml, renders the template with placeholder URL and
hash, and fails unless `bin` lists `bin/<name>.exe` for every
`release_binaries` entry. A kit bump that changes the expression fails here
until this script is reviewed.

Usage: check-scoop-manifest.py <kit-dir> <renderer-python> <renderer-cli>
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

MANIFEST = Path("release/publish-artifacts.toml")
WORKFLOW = Path("plugins/sc-publish/.github/workflows/scoop-publish.yml")
# Captures the parenthesised value of `"binary": ( ... ),` in the kit step.
BINARY_EXPR = re.compile(r'"binary":\s*(\(\s*.*?\n\s*\)),\n', re.DOTALL)


def fail(message: str) -> None:
    sys.exit(f"check-scoop-manifest: {message}")


def kit_binary_expression(kit: Path) -> str:
    text = (kit / WORKFLOW).read_text(encoding="utf-8")
    match = BINARY_EXPR.search(text)
    if match is None:
        fail(f"cannot find the 'binary' variable in {kit / WORKFLOW}; the kit step changed, review this check")
    return match.group(1)


def main() -> None:
    if len(sys.argv) != 4:
        fail(__doc__.strip().splitlines()[-1])
    kit, renderer_python, renderer_cli = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    channel = manifest["channels"]["scoop"]
    expected = [f"bin/{binary['name']}.exe" for binary in manifest["release_binaries"]]
    if len(expected) < 2:
        fail("release_binaries lists fewer than two binaries; nothing to shim twice")

    # Evaluate only the kit's own expression, with no builtins or other names.
    # The input is the workflow at the exact kit SHA in release/sc-publish-pin.toml.
    binary = eval(kit_binary_expression(kit), {"__builtins__": {}}, {"channel": channel})  # noqa: S307
    variables = {
        "version": "0.0.0",
        "description": manifest["project"]["description"],
        "homepage": manifest["project"]["homepage"],
        "license": manifest["project"]["license"],
        "windows_url": "https://example.invalid/sc-agent.zip",
        "windows_sha256": "0" * 64,
        "extract_dir": "sc-agent_0.0.0_x86_64-pc-windows-msvc",
        "binary": binary,
    }
    with tempfile.TemporaryDirectory() as tmp:
        var_file, output = Path(tmp, "vars.json"), Path(tmp, "manifest.json")
        var_file.write_text(json.dumps(variables), encoding="utf-8")
        subprocess.run(
            [
                renderer_python, renderer_cli, "render", "--mode", "file",
                "--root", ".", "--file", channel["manifest_template"],
                "--var-file", str(var_file), "--output", str(output),
            ],
            check=True,
        )
        rendered = json.loads(output.read_text(encoding="utf-8"))
    bins = rendered["architecture"]["64bit"]["bin"]
    if bins != expected:
        fail(f"rendered Scoop manifest bin is {bins!r}, expected one shim per binary: {expected!r}")
    print(f"scoop manifest shims: {', '.join(bins)}")


if __name__ == "__main__":
    main()
