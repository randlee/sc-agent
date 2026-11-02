"""Classify whether changed paths require the runtime CI jobs.

A documentation-only change keeps the required check names and skips crate
tests and template generation. The same rule is used by atm-core CI.
"""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Iterable


RUNTIME_PREFIXES = (
    ".github/",
    ".sc-lint/",
    "boundaries/",
    "crates/",
    "dev/",
    "release/",
    "scripts/",
    "template/",
    "tests/",
    "wizard/",
)
RUNTIME_FILES = {
    "Cargo.lock",
    "Cargo.toml",
    "Justfile",
    "deny.toml",
    "install.py",
    "requirements-generation.txt",
    "sc-lint.toml",
}


def requires_runtime(changed_paths: Iterable[str]) -> bool:
    """Return whether any path can change the Rust build, template, or CI."""
    return any(
        path in RUNTIME_FILES
        or path.startswith("rust-toolchain")
        or path.startswith(RUNTIME_PREFIXES)
        for path in changed_paths
        if path
    )


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: ci_scope.py <range-start> <head>", file=sys.stderr)
        return 2
    changed = subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", "-z", argv[1], argv[2]],
        check=True,
        capture_output=True,
    ).stdout
    paths = changed.decode("utf-8", errors="surrogateescape").split("\0")
    print(f"runtime={str(requires_runtime(paths)).lower()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
