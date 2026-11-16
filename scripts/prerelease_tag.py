#!/usr/bin/env python3
"""Choose an unused version, bump manifests if necessary, and push one new prerelease tag.

--dry-run is local-only and changes nothing. No explicit version is accepted.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def git(*args: str, capture: bool = True) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=capture, check=True)
    return result.stdout.strip() if capture else ""


def next_version(current: str, tags: set[str]) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", current):
        raise ValueError("workspace version must be major.minor.patch")
    major, minor, patch = map(int, current.split("."))
    used = {tag.removeprefix("prerelease/").removeprefix("v") for tag in tags}
    while f"{major}.{minor}.{patch}" in used:
        patch += 1
    return f"{major}.{minor}.{patch}"


def manifests() -> list[Path]:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
    paths = [ROOT / "Cargo.toml"]
    for member in workspace["members"]:
        paths.append(ROOT / member / "Cargo.toml")
        python = ROOT / member / "pyproject.toml"
        if python.exists():
            paths.append(python)
    return paths


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    manifest = tomllib.loads((ROOT / "release/publish-artifacts.toml").read_text())
    policy = manifest["prerelease"]
    branch = git("branch", "--show-current")
    if not branch or branch in policy["protected_branches"]:
        parser.error("refusing a protected or detached branch")
    if git("status", "--porcelain"):
        parser.error("commit or restore all work before creating a prerelease")
    if not args.dry_run:
        git("fetch", "origin", "--tags", capture=False)
    tags = set(git("tag", "--list").splitlines())
    if not args.dry_run:
        releases = subprocess.run(["gh", "release", "list", "--limit", "1000", "--json", "tagName"], cwd=ROOT, text=True, capture_output=True, check=True)
        tags.update(release["tagName"] for release in json.loads(releases.stdout))
    current = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    version = next_version(current, tags)
    tag = policy["tag_prefix"] + version
    if args.dry_run:
        print(f"would select {tag}; workspace {current}; version bump {version != current}")
        return 0
    if version != current:
        changed = []
        pattern = re.compile(r'(\bversion\s*=\s*")' + re.escape(current) + r'(")')
        for path in manifests():
            before = path.read_text()
            after = pattern.sub(lambda match: match[1] + version + match[2], before)
            if before != after:
                path.write_text(after)
                changed.append(str(path.relative_to(ROOT)))
        subprocess.run(["cargo", "update", "--workspace"], cwd=ROOT, check=True)
        git("add", "--", *changed, "Cargo.lock", capture=False)
        git("commit", "-m", f"release: prepare prerelease {version}", capture=False)
    # No force option: a racing publication or tag creation must fail closed.
    git("tag", "-a", tag, "-m", f"Prerelease {version}", capture=False)
    git("push", "origin", f"refs/tags/{tag}", capture=False)
    print(f"created and pushed {tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
